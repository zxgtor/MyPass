#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod i18n;
mod vault;
use i18n::t;

use eframe::egui::{self, Color32, RichText, Vec2};
use keepass::db::EntryId;
use std::{
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};
use vault::{Draft, Vault};
use zeroize::{Zeroize, Zeroizing};

const TEAL: Color32 = Color32::from_rgb(13, 132, 140);
const MUTED: Color32 = Color32::from_rgb(102, 119, 133);
const INK: Color32 = Color32::from_rgb(24, 39, 52);
const PALE: Color32 = Color32::from_rgb(231, 245, 246);

fn main() -> eframe::Result {
    i18n::init();
    eframe::run_native(
        "MyPass",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([880.0, 660.0])
                .with_min_inner_size([620.0, 460.0]),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(MyPass::new(cc)))),
    )
}

struct MyPass {
    vault: Option<Vault>,
    path: Option<PathBuf>,
    password: Zeroizing<String>,
    confirm: Zeroizing<String>,
    creating: bool,
    reveal: bool,
    search: String,
    editor: Option<Draft>,
    deleting: Option<EntryId>,
    settings: bool,
    error: String,
    notice: String,
    notice_at: Instant,
    activity: Instant,
    clipboard: Option<arboard::Clipboard>,
    copied: Option<(Zeroizing<String>, Instant)>,
    job: Option<mpsc::Receiver<vault::Result<Vault>>>,
    preview: bool,
    wipe_ui: bool,
}

impl MyPass {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        if let Ok(bytes) = std::fs::read("C:\\Windows\\Fonts\\msyh.ttc") {
            fonts
                .font_data
                .insert("chinese".into(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .get_mut(&egui::FontFamily::Proportional)
                .unwrap()
                .insert(0, "chinese".into());
        }
        cc.egui_ctx.set_fonts(fonts);
        cc.egui_ctx
            .send_viewport_cmd(egui::ViewportCommand::SetTheme(egui::SystemTheme::Light));
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals = egui::Visuals::light();
        style.visuals.panel_fill = Color32::WHITE;
        style.visuals.window_fill = Color32::WHITE;
        style.visuals.override_text_color = Some(INK);
        style.visuals.selection.bg_fill = PALE;
        style.visuals.selection.stroke.color = TEAL;
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(247, 249, 250);
        style.visuals.extreme_bg_color = Color32::from_rgb(248, 250, 251);
        style.visuals.widgets.inactive.bg_stroke =
            egui::Stroke::new(1.0_f32, Color32::from_rgb(211, 222, 228));
        style.visuals.widgets.hovered.bg_fill = PALE;
        style.spacing.item_spacing = Vec2::new(12.0, 12.0);
        style.spacing.button_padding = Vec2::new(18.0, 10.0);
        style.spacing.interact_size.y = 38.0;
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(16.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(16.0));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(26.0));
        cc.egui_ctx.set_style(style);
        let path = std::env::args()
            .nth(1)
            .filter(|s| !s.starts_with("--"))
            .map(PathBuf::from)
            .or_else(|| {
                std::fs::read_to_string(config_path())
                    .ok()
                    .map(PathBuf::from)
                    .filter(|p| p.exists())
            });
        let app = Self {
            vault: None,
            path,
            password: Zeroizing::new(String::new()),
            confirm: Zeroizing::new(String::new()),
            creating: false,
            reveal: false,
            search: String::new(),
            editor: None,
            deleting: None,
            settings: false,
            error: String::new(),
            notice: String::new(),
            notice_at: Instant::now(),
            activity: Instant::now(),
            clipboard: arboard::Clipboard::new().ok(),
            copied: None,
            job: None,
            preview: false,
            wipe_ui: false,
        };
        #[cfg(debug_assertions)]
        {
            let mut app = app;
            if std::env::args().any(|a| a == "--preview") {
                app.preview = true;
                app.vault = Some(Vault::preview());
                if std::env::args().any(|a| a == "--editor") {
                    app.editor = Some(Draft {
                        token: true,
                        ..Default::default()
                    });
                }
            }
            app
        }
        #[cfg(not(debug_assertions))]
        app
    }

    fn spawn(
        &mut self,
        ctx: &egui::Context,
        work: impl FnOnce() -> vault::Result<Vault> + Send + 'static,
    ) {
        if self.preview {
            self.error = t("当前是界面预览，不会保存示例数据。").into();
            return;
        }
        let (tx, rx) = mpsc::channel();
        let context = ctx.clone();
        self.job = Some(rx);
        self.error.clear();
        std::thread::spawn(move || {
            let _ = tx.send(work());
            context.request_repaint();
        });
    }

    fn poll(&mut self) {
        let result = self.job.as_ref().and_then(|rx| match rx.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(_) => Some(Err(t("操作未完成，请重试。").into())),
        });
        if let Some(result) = result {
            self.job = None;
            match result {
                Ok(vault) => {
                    self.path = Some(vault.path.clone());
                    if let Some(parent) = config_path().parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::write(config_path(), vault.path.to_string_lossy().as_bytes());
                    self.vault = Some(vault);
                    self.editor = None;
                    self.deleting = None;
                    self.password.zeroize();
                    self.confirm.zeroize();
                    self.reveal = false;
                    self.activity = Instant::now();
                    self.toast(t("已保存"));
                }
                Err(e) => self.error = e,
            }
        }
    }

    fn toast(&mut self, text: &str) {
        self.notice = text.into();
        self.notice_at = Instant::now();
    }

    fn copy(&mut self, value: String) {
        let secret = Zeroizing::new(value);
        let result = self
            .clipboard
            .as_mut()
            .ok_or(t("无法访问剪贴板").to_string())
            .and_then(|c| {
                c.set_text(secret.as_str())
                    .map_err(|_| t("复制失败，请重试").into())
            });
        match result {
            Ok(()) => {
                self.copied = Some((secret, Instant::now()));
                self.toast(t("已复制 · 30 秒后清除剪贴板"));
            }
            Err(e) => self.error = e,
        }
    }

    fn clear_clipboard(&mut self) {
        if let Some((text, _)) = self.copied.take()
            && let Some(c) = self.clipboard.as_mut()
            && c.get_text()
                .is_ok_and(|current| Zeroizing::new(current).as_str() == text.as_str())
        {
            let _ = c.clear();
        }
    }

    fn lock(&mut self) {
        self.clear_clipboard();
        self.vault = None;
        self.editor = None;
        self.deleting = None;
        self.password.zeroize();
        self.confirm.zeroize();
        self.search.zeroize();
        self.reveal = false;
        self.settings = false;
        self.creating = false;
        self.error.clear();
        self.wipe_ui = true;
    }

    fn picker(&mut self, create: bool) {
        let dialog = rfd::FileDialog::new().add_filter(t("KeePass 密码库"), &["kdbx"]);
        let path = if create {
            dialog.set_file_name("MyPass.kdbx").save_file()
        } else {
            dialog.pick_file()
        };
        if let Some(p) = path {
            self.path = Some(p);
            self.creating = create;
            self.error.clear();
            self.password.zeroize();
            self.confirm.zeroize();
            self.reveal = false;
        }
    }

    fn locked(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space((ui.available_height() * 0.12).max(20.0));
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("MyPass").size(36.0).strong().color(TEAL));
            ui.add_space(6.0);
            ui.heading(if self.creating {
                t("创建你的密码库")
            } else if self.path.is_some() {
                t("欢迎回来")
            } else {
                t("密码和 Token，放在一起")
            });
            ui.label(RichText::new(t("保存在你的电脑上，由主密码解锁")).color(MUTED));
            ui.add_space(26.0);
            ui.allocate_ui_with_layout(
                Vec2::new(370.0_f32.min(ui.available_width()), 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    if let Some(path) = &self.path {
                        ui.label(
                            RichText::new(path.file_name().unwrap_or_default().to_string_lossy())
                                .color(MUTED),
                        )
                        .on_hover_text(path.display().to_string());
                        ui.label(t("主密码"));
                        let input = ui.add(
                            egui::TextEdit::singleline(&mut *self.password)
                                .password(!self.reveal)
                                .desired_width(f32::INFINITY)
                                .margin(egui::Margin::same(12)),
                        );
                        if self.creating {
                            ui.label(t("再次输入主密码"));
                            ui.add(
                                egui::TextEdit::singleline(&mut *self.confirm)
                                    .password(!self.reveal)
                                    .desired_width(f32::INFINITY)
                                    .margin(egui::Margin::same(12)),
                            );
                            ui.label(
                                RichText::new(t("至少 12 个字符。忘记后无法找回，请妥善记住。"))
                                    .small()
                                    .color(MUTED),
                            );
                        }
                        ui.checkbox(&mut self.reveal, t("显示主密码"));
                        let submit = ui
                            .add_sized(
                                [ui.available_width(), 44.0],
                                primary(if self.creating {
                                    t("创建密码库")
                                } else {
                                    t("解锁")
                                }),
                            )
                            .clicked()
                            || (input.lost_focus()
                                && ctx.input(|i| i.key_pressed(egui::Key::Enter)));
                        if submit {
                            if self.creating && self.password.as_str() != self.confirm.as_str() {
                                self.error = t("两次输入的主密码不一致。").into();
                            } else if self.password.is_empty() {
                                self.error = t("请输入主密码。").into();
                            } else {
                                let path = self.path.clone().unwrap();
                                let pass = self.password.clone();
                                let create = self.creating;
                                self.spawn(ctx, move || {
                                    if create {
                                        Vault::create(path, &pass)
                                    } else {
                                        Vault::open(path, &pass)
                                    }
                                });
                            }
                        }
                        ui.add_space(8.0);
                    }
                    if self.path.is_none() {
                        if ui
                            .add_sized([ui.available_width(), 44.0], primary(t("创建我的密码库")))
                            .clicked()
                        {
                            self.picker(true);
                        }
                        ui.add_space(8.0);
                    }
                    ui.horizontal(|ui| {
                        if ui.link(t("打开密码库")).clicked() {
                            self.picker(false);
                        }
                        if self.path.is_some() {
                            ui.label(RichText::new("·").color(MUTED));
                            if ui.link(t("新建密码库")).clicked() {
                                self.picker(true);
                            }
                        }
                    });
                },
            );
        });
    }

    fn list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_sized(
                [ui.available_width() - 116.0, 44.0],
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text(t("搜索名称、账号或网址"))
                    .margin(egui::Margin::same(12)),
            );
            if ui.add_sized([104.0, 44.0], primary(t("＋ 添加"))).clicked() {
                self.editor = Some(Draft::default());
                self.reveal = false;
                self.error.clear();
            }
        });
        ui.add_space(14.0);
        let query = self.search.to_lowercase();
        let db = &self.vault.as_ref().unwrap().db;
        let recycle = db.recycle_bin().map(|g| g.id());
        let mut rows: Vec<_> = db
            .iter_all_entries()
            .filter(|e| {
                let mut parent = Some(e.parent().id());
                while let Some(id) = parent {
                    if Some(id) == recycle {
                        return false;
                    }
                    parent = db.group(id).and_then(|g| g.parent().map(|p| p.id()));
                }
                format!(
                    "{} {} {}",
                    e.get_title().unwrap_or_default(),
                    e.get_username().unwrap_or_default(),
                    e.get_url().unwrap_or_default()
                )
                .to_lowercase()
                .contains(&query)
            })
            .map(|e| {
                (
                    e.id(),
                    e.get_title().unwrap_or(t("未命名")).to_string(),
                    e.get_username().unwrap_or_default().to_string(),
                    e.get("MyPass.Type") == Some("token"),
                )
            })
            .collect();
        rows.sort_by_key(|r| r.1.to_lowercase());
        if rows.is_empty() {
            ui.add_space(100.0);
            ui.vertical_centered(|ui| {
                ui.heading(if query.is_empty() {
                    t("添加第一个账号或 Token")
                } else {
                    t("没有找到匹配项")
                });
                ui.label(
                    RichText::new(if query.is_empty() {
                        t("点击右上角「添加」，下次取用只需一键复制。")
                    } else {
                        t("试试名称、用户名或网址。")
                    })
                    .color(MUTED),
                );
            });
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (id, title, username, token) in rows {
                    let width = ui.available_width();
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::new(width, 86.0), egui::Sense::hover());
                    let hovered = ui.rect_contains_pointer(rect);
                    if hovered {
                        ui.painter()
                            .rect_filled(rect, 8.0, Color32::from_rgb(244, 250, 250));
                    }
                    let icon_rect = egui::Rect::from_min_size(
                        rect.min + Vec2::new(12.0, 21.0),
                        Vec2::splat(44.0),
                    );
                    ui.painter().rect_filled(icon_rect, 12.0, PALE);
                    ui.painter().text(
                        icon_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        if token {
                            "API".into()
                        } else {
                            title
                                .chars()
                                .next()
                                .unwrap_or('?')
                                .to_uppercase()
                                .to_string()
                        },
                        egui::FontId::proportional(if token { 14.0 } else { 23.0 }),
                        TEAL,
                    );
                    let label_rect = egui::Rect::from_min_max(
                        rect.min + Vec2::new(72.0, 12.0),
                        egui::pos2(rect.right() - 236.0, rect.bottom() - 10.0),
                    );
                    let response =
                        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(label_rect), |ui| {
                            ui.add(
                                egui::Label::new(RichText::new(&title).size(19.0).strong())
                                    .truncate(),
                            );
                            ui.add(
                                egui::Label::new(
                                    RichText::new(if token {
                                        "API Token"
                                    } else if username.is_empty() {
                                        t("账号密码")
                                    } else {
                                        &username
                                    })
                                    .color(MUTED),
                                )
                                .truncate(),
                            );
                        });
                    if ui
                        .interact(
                            response.response.rect,
                            ui.id().with(id),
                            egui::Sense::click(),
                        )
                        .on_hover_text(t("查看或编辑"))
                        .clicked()
                    {
                        self.editor = self.vault.as_ref().unwrap().draft(id);
                        self.reveal = false;
                    }
                    let actions = egui::Rect::from_min_size(
                        egui::pos2(rect.right() - 224.0, rect.top() + 21.0),
                        Vec2::new(224.0, 44.0),
                    );
                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(actions), |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().button_padding.x = 12.0;
                            if ui
                                .add_sized(
                                    [164.0, 42.0],
                                    egui::Button::new(
                                        RichText::new(if token {
                                            t("复制 Token")
                                        } else {
                                            t("复制密码")
                                        })
                                        .color(if hovered { Color32::WHITE } else { TEAL }),
                                    )
                                    .fill(if hovered { TEAL } else { PALE })
                                    .truncate()
                                    .corner_radius(7.0),
                                )
                                .clicked()
                            {
                                let value = self
                                    .vault
                                    .as_ref()
                                    .unwrap()
                                    .db
                                    .entry(id)
                                    .and_then(|e| e.get_password().map(str::to_owned));
                                if let Some(value) = value {
                                    self.copy(value);
                                } else {
                                    self.error = t("此条目尚未设置密码。").into();
                                }
                            }
                            ui.menu_button("···", |ui| {
                                if ui.button(t("查看 / 编辑")).clicked() {
                                    self.editor = self.vault.as_ref().unwrap().draft(id);
                                    self.reveal = false;
                                    ui.close_menu();
                                }
                                if !token && ui.button(t("复制用户名")).clicked() {
                                    self.copy(username.clone());
                                    ui.close_menu();
                                }
                                if ui.button(t("打开网站")).clicked() {
                                    let url = self
                                        .vault
                                        .as_ref()
                                        .unwrap()
                                        .db
                                        .entry(id)
                                        .and_then(|e| e.get_url().map(str::to_owned))
                                        .unwrap_or_default();
                                    if url.starts_with("https://") || url.starts_with("http://") {
                                        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                                    } else {
                                        self.error =
                                            t("请在编辑中填写以 https:// 或 http:// 开头的网址。")
                                                .into();
                                    }
                                    ui.close_menu();
                                }
                                ui.separator();
                                if ui
                                    .button(
                                        RichText::new(t("删除"))
                                            .color(Color32::from_rgb(180, 50, 55)),
                                    )
                                    .clicked()
                                {
                                    self.deleting = Some(id);
                                    ui.close_menu();
                                }
                            });
                        });
                    });
                    ui.painter().hline(
                        rect.x_range(),
                        rect.bottom(),
                        egui::Stroke::new(1.0_f32, Color32::from_rgb(237, 241, 243)),
                    );
                }
            });
    }

    fn edit_window(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.editor.take() else {
            return;
        };
        let mut open = true;
        let mut save = false;
        let mut cancel = false;
        egui::Window::new(if draft.id.is_some() {
            t("编辑条目")
        } else {
            t("添加条目")
        })
        .id(egui::Id::new("editor"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(440.0)
        .max_height((ctx.screen_rect().height() - 80.0).max(240.0))
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.add_enabled_ui(self.job.is_none(), |ui| {
                egui::ScrollArea::vertical()
                    .max_height((ctx.screen_rect().height() - 210.0).max(160.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut draft.token, false, t("账号密码"));
                            ui.selectable_value(&mut draft.token, true, "API Token");
                        });
                        ui.add_space(4.0);
                        field(ui, t("名称"), &mut draft.title, false);
                        if !draft.token {
                            field(ui, t("用户名"), &mut draft.username, false);
                        }
                        field(
                            ui,
                            if draft.token { "Token" } else { t("密码") },
                            &mut draft.secret,
                            !self.reveal,
                        );
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.reveal, t("显示内容"));
                            if !draft.token && ui.small_button(t("生成密码")).clicked() {
                                use rand::Rng;
                                const CHARS: &[u8] =
                            b"abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789!@#$%&*-_+";
                                let mut rng = rand::rngs::OsRng;
                                *draft.secret = (0..24)
                                    .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
                                    .collect();
                            }
                        });
                        field(ui, t("网址（可选）"), &mut draft.url, false);
                        ui.label(t("备注（可选）"));
                        ui.add(
                            egui::TextEdit::multiline(&mut *draft.notes)
                                .desired_rows(2)
                                .desired_width(f32::INFINITY),
                        );
                    });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    save = ui.add(primary(t("保存"))).clicked();
                    cancel = ui.button(t("取消")).clicked();
                });
            });
            if !self.error.is_empty() {
                ui.colored_label(Color32::DARK_RED, &self.error);
            }
        });
        if self.job.is_some() {
            self.editor = Some(draft);
            return;
        }
        if save {
            if draft.title.trim().is_empty() || draft.secret.is_empty() {
                self.error = t("请填写名称和密码 / Token。").into();
                self.editor = Some(draft);
            } else {
                let mut vault = self.vault.as_ref().unwrap().clone();
                // Keep the form until disk persistence has succeeded.
                let worker_draft = Draft {
                    id: draft.id,
                    token: draft.token,
                    title: draft.title.clone(),
                    username: draft.username.clone(),
                    secret: draft.secret.clone(),
                    url: draft.url.clone(),
                    notes: draft.notes.clone(),
                };
                self.editor = Some(draft);
                self.spawn(ctx, move || {
                    vault.put(&worker_draft)?;
                    Ok(vault)
                });
            }
        } else if open && !cancel {
            self.editor = Some(draft);
        } else {
            self.error.clear();
            self.reveal = false;
        }
    }
}

impl eframe::App for MyPass {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        if self.wipe_ui {
            ctx.memory_mut(|m| m.data.clear());
            self.wipe_ui = false;
        }
        ctx.request_repaint_after(Duration::from_millis(250));
        if ctx.input(|i| !i.events.is_empty()) {
            self.activity = Instant::now();
        }
        if self
            .copied
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() >= Duration::from_secs(30))
        {
            self.clear_clipboard();
        }
        if self.vault.is_some()
            && self.job.is_none()
            && (self.activity.elapsed() > Duration::from_secs(300)
                || ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::L)))
        {
            self.lock();
        }
        if self.job.is_some() && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        egui::TopBottomPanel::bottom("footer")
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(250, 252, 252))
                    .inner_margin(egui::Margin::symmetric(24, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.job.is_some() {
                        ui.spinner();
                        ui.label(t("正在加密处理，请稍候…"));
                    } else {
                        ui.label(
                            RichText::new(if self.preview {
                                t("界面预览 · 示例数据，不保存")
                            } else if !self.notice.is_empty()
                                && self.notice_at.elapsed() < Duration::from_secs(5)
                            {
                                &self.notice
                            } else if self.vault.is_some() {
                                t("● 已保存")
                            } else {
                                t("本地加密密码库")
                            })
                            .color(if self.vault.is_some() {
                                TEAL
                            } else {
                                MUTED
                            }),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.vault.is_some() {
                            if ui
                                .add_enabled(self.job.is_none(), egui::Button::new(t("锁定")))
                                .clicked()
                            {
                                self.lock();
                            }
                            if ui.button(t("设置")).clicked() {
                                self.settings = true;
                            }
                        }
                    });
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::WHITE).inner_margin(24))
            .show(ctx, |ui| {
                if !self.error.is_empty() && self.editor.is_none() {
                    ui.colored_label(Color32::DARK_RED, &self.error);
                    ui.add_space(8.0);
                }
                ui.add_enabled_ui(
                    self.job.is_none()
                        && self.editor.is_none()
                        && self.deleting.is_none()
                        && !self.settings,
                    |ui| {
                        if self.vault.is_some() {
                            self.list(ui);
                        } else {
                            self.locked(ui, ctx);
                        }
                    },
                );
            });
        self.edit_window(ctx);
        if let Some(id) = self.deleting {
            egui::Window::new(t("删除条目？"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(t("此条目将从密码库删除。保存前会备份原文件。"));
                    ui.add_enabled_ui(self.job.is_none(), |ui| {
                        ui.horizontal(|ui| {
                            if ui.button(t("取消")).clicked() {
                                self.deleting = None;
                            }
                            if ui.button(t("确认删除")).clicked() {
                                let mut vault = self.vault.as_ref().unwrap().clone();
                                self.spawn(ctx, move || {
                                    vault.delete(id)?;
                                    Ok(vault)
                                });
                            }
                        });
                    });
                    if !self.error.is_empty() {
                        ui.colored_label(Color32::DARK_RED, &self.error);
                    }
                });
        }
        if self.settings {
            let mut open = true;
            egui::Window::new(t("设置"))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(420.0)
                .show(ctx, |ui| {
                    ui.label(t("5 分钟无操作后自动锁定"));
                    ui.label(t("复制后 30 秒自动清除剪贴板"));
                    ui.label(t("每次保存前保留上一版 .kdbx.bak 备份"));
                    ui.separator();
                    if let Some(path) = &self.path {
                        ui.label(t("密码库位置"));
                        ui.label(path.display().to_string());
                    }
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(t("请另行备份密码库和主密码。首版尚未经过独立安全审计。"))
                            .color(MUTED),
                    );
                    if ui.button(t("锁定并切换密码库")).clicked() {
                        self.lock();
                        self.path = None;
                    }
                });
            if !open {
                self.settings = false;
            }
        }
    }
}

impl Drop for MyPass {
    fn drop(&mut self) {
        self.clear_clipboard();
    }
}

fn primary(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).color(Color32::WHITE))
        .fill(TEAL)
        .corner_radius(7.0)
}
fn field(ui: &mut egui::Ui, label: &str, value: &mut String, secret: bool) {
    ui.label(label);
    ui.add(
        egui::TextEdit::singleline(value)
            .password(secret)
            .desired_width(f32::INFINITY)
            .margin(egui::Margin::same(10)),
    );
}
fn config_path() -> PathBuf {
    PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_else(|| ".".into()))
        .join("MyPass")
        .join("last-vault.txt")
}
