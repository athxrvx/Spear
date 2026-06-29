use gtk4::prelude::*;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::plugins::ResultItem;
use crate::plugin_manager::PluginManager;
use crate::utils::{copy_to_clipboard, launch_url};
use crate::settings::{AppSettings, apply_color_scheme};
use crate::layer_shell::setup_layer_shell;

fn load_custom_theme(theme_name: &str) -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let path = std::path::PathBuf::from(home)
        .join(".config")
        .join("spear")
        .join("themes")
        .join(format!("{}.json", theme_name));
    
    if !path.exists() {
        return None;
    }
    
    let content = std::fs::read_to_string(path).ok()?;
    let colors: serde_json::Value = serde_json::from_str(&content).ok()?;
    
    let window_bg = colors.get("window_bg_color")?.as_str()?;
    let window_fg = colors.get("window_fg_color")?.as_str()?;
    let accent_bg = colors.get("accent_bg_color")?.as_str()?;
    let accent_fg = colors.get("accent_fg_color")?.as_str()?;
    let popover_bg = colors.get("popover_bg_color")?.as_str()?;
    let popover_fg = colors.get("popover_fg_color")?.as_str()?;
    let text_color = colors.get("text_color")?.as_str()?;
    let entry_bg = colors.get("entry_bg_color")
        .and_then(|v| v.as_str())
        .unwrap_or(popover_bg);
    
    Some(format!(
        "\n@define-color window_bg_color {};\n\
         @define-color window_fg_color {};\n\
         @define-color accent_bg_color {};\n\
         @define-color accent_fg_color {};\n\
         @define-color popover_bg_color {};\n\
         @define-color popover_fg_color {};\n\
         @define-color entry_bg_color {};\n\
         @define-color text_color {};\n",
        window_bg, window_fg, accent_bg, accent_fg, popover_bg, popover_fg, entry_bg, text_color
    ))
}

fn generate_css(search_font_size: i32, title_font_size: i32, theme: &str, color_scheme: &str, window_width: i32) -> String {
    let theme_colors = match theme {
        "tokyonight" => "
@define-color window_bg_color #1a1b26;
@define-color window_fg_color #c0caf5;
@define-color accent_bg_color #7aa2f7;
@define-color accent_fg_color #15161e;
@define-color popover_bg_color #1f2335;
@define-color popover_fg_color #c0caf5;
@define-color entry_bg_color #24283b;
@define-color text_color #9aa5ce;".to_string(),
        "dracula" => "
@define-color window_bg_color #282a36;
@define-color window_fg_color #f8f8f2;
@define-color accent_bg_color #bd93f9;
@define-color accent_fg_color #282a36;
@define-color popover_bg_color #44475a;
@define-color popover_fg_color #f8f8f2;
@define-color entry_bg_color #343746;
@define-color text_color #6272a4;".to_string(),
        "catppuccin" => "
@define-color window_bg_color #1e1e2e;
@define-color window_fg_color #cdd6f4;
@define-color accent_bg_color #cba6f7;
@define-color accent_fg_color #11111b;
@define-color popover_bg_color #181825;
@define-color popover_fg_color #cdd6f4;
@define-color entry_bg_color #2a2b3c;
@define-color text_color #a6adc8;".to_string(),
        "gruvbox" => "
@define-color window_bg_color #282828;
@define-color window_fg_color #ebdbb2;
@define-color accent_bg_color #fabd2f;
@define-color accent_fg_color #282828;
@define-color popover_bg_color #3c3836;
@define-color popover_fg_color #ebdbb2;
@define-color entry_bg_color #32302f;
@define-color text_color #a89984;".to_string(),
        "jellybeans" => "
@define-color window_bg_color #151515;
@define-color window_fg_color #e8e8d3;
@define-color accent_bg_color #8197bf;
@define-color accent_fg_color #151515;
@define-color popover_bg_color #303030;
@define-color popover_fg_color #e8e8d3;
@define-color entry_bg_color #202020;
@define-color text_color #888888;".to_string(),
        "min" => {
            let is_dark = if color_scheme == "default" {
                libadwaita::StyleManager::default().is_dark()
            } else {
                color_scheme == "dark"
            };
            if is_dark {
                "
@define-color window_bg_color #121212;
@define-color window_fg_color #eeeeee;
@define-color accent_bg_color #222222;
@define-color accent_fg_color #ffffff;
@define-color popover_bg_color #1a1a1a;
@define-color popover_fg_color #eeeeee;
@define-color entry_bg_color #1e1e1e;
@define-color text_color #999999;".to_string()
            } else {
                "
@define-color window_bg_color #ffffff;
@define-color window_fg_color #111111;
@define-color accent_bg_color #f0f0f0;
@define-color accent_fg_color #111111;
@define-color popover_bg_color #fafafa;
@define-color popover_fg_color #111111;
@define-color entry_bg_color #f5f5f5;
@define-color text_color #666666;".to_string()
            }
        },
        other => {
            load_custom_theme(other).unwrap_or_default()
        }
    };

    let mut theme_colors = theme_colors;
    if !theme_colors.contains("@define-color entry_bg_color") {
        theme_colors = format!("@define-color entry_bg_color @popover_bg_color;\n{}", theme_colors);
    }
    if !theme_colors.contains("@define-color text_color") {
        theme_colors = format!("@define-color text_color @window_fg_color;\n{}", theme_colors);
    }

    format!(
        concat!("{}",
        r#"
window.launcher-window,
.launcher-window {{
    background: transparent;
    background-color: transparent;
    background-image: none;
    box-shadow: none;
    border: none;
}}

.launcher-root {{
    background-color: @window_bg_color;
    color: @window_fg_color;
    border-radius: 12px;
    border: 1px solid alpha(@window_fg_color, 0.08);
    font-family: "Adwaita Sans", Cantarell, sans-serif;
    min-width: {3}px;
    box-shadow: none;
}}

.search-container {{
    background-color: @window_bg_color;
    border-bottom: 1px solid alpha(@window_fg_color, 0.05);
    padding: 8px 16px;
    border-top-left-radius: 11px;
    border-top-right-radius: 11px;
}}

.launcher-root entry {{
    background-color: @entry_bg_color;
    border: 1px solid alpha(@window_fg_color, 0.1);
    border-radius: 8px;
    padding: 0px 12px;
    box-shadow: none;
    font-size: {1}px;
    color: @window_fg_color;
    caret-color: @accent_bg_color;
    margin-left: 8px;
    margin-right: 0px;
}}

.launcher-root entry:focus {{
    border-color: @accent_bg_color;
    box-shadow: none;
}}

.launcher-root entry.focused-mode {{
    margin-left: 0px;
    margin-right: 0px;
}}

.launcher-root list.minimal-mode row {{
    padding: 4px 10px;
    margin-bottom: 2px;
}}

.launcher-root list {{
    background-color: transparent;
    padding: 8px;
    min-width: 0;
}}

.launcher-root row {{
    background-color: transparent;
    border-radius: 8px;
    padding: 8px 12px;
    margin-bottom: 4px;
    transition: background-color 0.1s ease;
    min-width: 0;
}}

.launcher-root row:hover {{
    background-color: alpha(@window_fg_color, 0.05);
}}

.launcher-root row:selected {{
    background-color: @accent_bg_color;
    color: @accent_fg_color;
}}

.launcher-root row image {{
    color: @window_fg_color;
}}

.launcher-root row:selected label {{
    color: @accent_fg_color;
}}

.launcher-root row:selected image {{
    color: @accent_fg_color;
}}

.row-title {{
    font-size: {2}px;
    font-weight: bold;
}}

.row-subtitle {{
    font-size: 12px;
    color: @text_color;
    opacity: 0.7;
}}

.launcher-root row:selected .row-subtitle {{
    color: @accent_fg_color;
    opacity: 0.85;
}}

.row-category {{
    font-size: 11px;
    color: @window_fg_color;
    background-color: alpha(@window_fg_color, 0.08);
    border-radius: 6px;
    padding: 3px 8px;
    font-weight: bold;
}}

.launcher-root row:selected .row-category {{
    background-color: alpha(@accent_fg_color, 0.2);
    color: @accent_fg_color;
}}

.footer {{
    background-color: @window_bg_color;
    border-top: 1px solid alpha(@window_fg_color, 0.05);
    padding: 10px 18px;
    border-bottom-left-radius: 11px;
    border-bottom-right-radius: 11px;
}}

.footer-status {{
    font-size: 12px;
    color: @text_color;
    opacity: 0.7;
}}

.footer-action-label {{
    font-size: 12px;
    color: @window_fg_color;
}}

.footer-action-btn {{
    background-color: alpha(@window_fg_color, 0.08);
    color: @window_fg_color;
    border-radius: 6px;
    padding: 4px 10px;
    font-size: 11px;
    font-weight: bold;
}}

.footer-action-btn:hover {{
    background-color: alpha(@window_fg_color, 0.15);
}}

.footer-icon-btn {{
    background: none;
    border: none;
    padding: 4px;
    border-radius: 6px;
    color: @window_fg_color;
    opacity: 0.6;
    transition: opacity 0.15s ease, background-color 0.15s ease;
}}

.footer-icon-btn:hover {{
    opacity: 1.0;
    background-color: alpha(@window_fg_color, 0.08);
}}

.preview-panel {{
    background-color: @window_bg_color;
    border-left: 1px solid alpha(@window_fg_color, 0.08);
}}

.preview-title {{
    font-size: 15px;
    font-weight: bold;
    color: @window_fg_color;
}}

.preview-subtitle {{
    font-size: 12px;
    color: @text_color;
    opacity: 0.7;
}}

.preview-metadata-label {{
    font-size: 11px;
    color: @text_color;
    opacity: 0.6;
}}

.preview-metadata-value {{
    font-size: 11px;
    color: @window_fg_color;
    font-weight: bold;
}}

.preview-text-view {{
    font-family: monospace;
    font-size: 11px;
    background-color: alpha(@window_fg_color, 0.03);
    border: 1px solid alpha(@window_fg_color, 0.05);
    border-radius: 6px;
    padding: 8px;
}}

.launcher-root popover {{
    background-color: @popover_bg_color;
    color: @popover_fg_color;
    border: 1px solid alpha(@popover_fg_color, 0.1);
    border-radius: 10px;
    padding: 6px;
}}

.launcher-root popover list {{
    padding: 0px;
}}

.launcher-root popover row {{
    border-radius: 6px;
    padding: 6px 12px;
}}

.launcher-root popover row:selected {{
    background-color: @accent_bg_color;
    color: @accent_fg_color;
}}
"#),
        theme_colors,
        search_font_size,
        title_font_size,
        window_width
    )
}

pub struct SpearWindow {

    state: Rc<RefCell<WindowState>>,
}

struct WindowState {
    window: libadwaita::ApplicationWindow,
    entry: gtk4::Entry,
    search_icon: gtk4::Image,
    listbox: gtk4::ListBox,
    flowbox: gtk4::FlowBox,
    scrolled_window: gtk4::ScrolledWindow,
    empty_label: gtk4::Label,
    status_label: gtk4::Label,
    action_btn: gtk4::Button,
    popover: gtk4::Popover,
    popover_list: gtk4::ListBox,
    spinner: gtk4::Spinner,

    plugin_manager: Arc<PluginManager>,
    results_by_plugin: HashMap<String, Vec<ResultItem>>,
    current_items: Vec<ResultItem>,
    channel_sender: async_channel::Sender<(String, Vec<ResultItem>, Option<String>)>,

    settings: Rc<RefCell<AppSettings>>,
    frecency_store: Rc<RefCell<crate::frecency::FrecencyStore>>,
    css_provider: gtk4::CssProvider,

    preview_panel: gtk4::Box,
    preview_icon: gtk4::Image,
    preview_title: gtk4::Label,
    preview_subtitle: gtk4::Label,
    preview_metadata: gtk4::Box,
    preview_text_scroll: gtk4::ScrolledWindow,
    preview_text: gtk4::TextView,
    preview_picture: gtk4::Picture,
}

impl SpearWindow {
    pub fn new(
        app: &libadwaita::Application,
        plugin_manager: Arc<PluginManager>,
        settings: Rc<RefCell<AppSettings>>,
    ) -> Self {
        let initial_width = settings.borrow().window_width;
        let initial_height = settings.borrow().window_height;

        // Create window — set_size_request is the only reliable way to lock
        // the size in GTK4; default_width/height are just hints and are ignored
        // once the window is realized. We set both dimensions here and update
        // them whenever settings change.
        let window = libadwaita::ApplicationWindow::builder()
            .application(app)
            .title("Spear Launcher")
            .resizable(false)
            .decorated(false)
            .build();
        window.set_size_request(initial_width, initial_height);
        window.add_css_class("launcher-window");

        // Set up as a Wayland layer-shell surface so the compositor centres it
        // and respects our fixed width. Falls back silently on X11 / unsupported compositors.
        setup_layer_shell(&window);

        // Load Dynamic CSS based on settings
        let provider = gtk4::CssProvider::new();
        let css_data = generate_css(
            settings.borrow().search_font_size,
            settings.borrow().title_font_size,
            &settings.borrow().theme,
            &settings.borrow().color_scheme,
            settings.borrow().window_width,
        );
        provider.load_from_data(&css_data);

        if let Some(display) = gdk4::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }

        // Apply theme color scheme
        apply_color_scheme(&settings.borrow().color_scheme);

        // 1. Build UI Elements
        let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        main_box.add_css_class("launcher-root");
        window.set_content(Some(&main_box));

        // Search Bar Container
        let search_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        search_box.add_css_class("search-container");
        search_box.set_hexpand(true);
        search_box.set_halign(gtk4::Align::Fill);
        main_box.append(&search_box);

        let search_icon = gtk4::Image::from_icon_name("system-search-symbolic");
        search_icon.set_pixel_size(18);
        search_icon.set_valign(gtk4::Align::Center);
        search_box.append(&search_icon);

        let entry = gtk4::Entry::builder()
            .placeholder_text("Search apps, calculate, search the web or run plugins...")
            .hexpand(true)
            .halign(gtk4::Align::Fill)
            .width_chars(0)
            .build();
        search_box.append(&entry);

        // Apply initial layout mode visibility/styles
        let initial_layout_mode = settings.borrow().layout_mode.clone();
        if initial_layout_mode == "focused" || initial_layout_mode == "minimal" {
            search_icon.set_visible(false);
        } else {
            search_icon.set_visible(true);
        }
        if initial_layout_mode == "focused" {
            entry.add_css_class("focused-mode");
        } else {
            entry.remove_css_class("focused-mode");
        }

        let spinner = gtk4::Spinner::builder()
            .width_request(18)
            .height_request(18)
            .valign(gtk4::Align::Center)
            .visible(false)
            .build();
        search_box.append(&spinner);

        // Content Box (contains Results List on left, Preview Panel on right)
        let content_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        content_box.set_vexpand(true);
        main_box.append(&content_box);

        // Results Container — propagate_natural_width(false) is the key GTK4
        // setting that stops the child ListBox's natural width from bubbling up
        // and making the window wider. hexpand(true) fills the fixed container.
        let scrolled_window = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .propagate_natural_width(false)
            .hexpand(true)
            .min_content_height(350)
            .build();
        content_box.append(&scrolled_window);

        let listbox = gtk4::ListBox::new();
        listbox.set_selection_mode(gtk4::SelectionMode::Single);
        if initial_layout_mode == "minimal" {
            listbox.add_css_class("minimal-mode");
        }
        
        let flowbox = gtk4::FlowBox::new();
        flowbox.set_selection_mode(gtk4::SelectionMode::Single);
        flowbox.set_max_children_per_line(4);
        flowbox.set_min_children_per_line(4);
        flowbox.set_homogeneous(true);
        flowbox.add_css_class("grid-view");

        if settings.borrow().grid_view_enabled {
            scrolled_window.set_child(Some(&flowbox));
        } else {
            scrolled_window.set_child(Some(&listbox));
        }

        // Preview Panel Container
        let preview_panel = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        preview_panel.add_css_class("preview-panel");
        preview_panel.set_width_request(260);
        preview_panel.set_visible(settings.borrow().file_preview_enabled);
        content_box.append(&preview_panel);

        let preview_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .build();
        preview_panel.append(&preview_scroll);

        let preview_content = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
        preview_content.set_margin_start(16);
        preview_content.set_margin_end(16);
        preview_content.set_margin_top(16);
        preview_content.set_margin_bottom(16);
        preview_scroll.set_child(Some(&preview_content));

        // Image Preview (at the top)
        let preview_picture = gtk4::Picture::builder()
            .can_shrink(true)
            .margin_top(8)
            .margin_bottom(8)
            .height_request(150)
            .hexpand(true)
            .visible(false)
            .build();
        preview_content.append(&preview_picture);

        // Big Icon
        let preview_icon = gtk4::Image::from_icon_name("system-run-symbolic");
        preview_icon.set_pixel_size(64);
        preview_icon.set_halign(gtk4::Align::Center);
        preview_content.append(&preview_icon);

        // Big Title
        let preview_title = gtk4::Label::new(None);
        preview_title.add_css_class("preview-title");
        preview_title.set_wrap(true);
        preview_title.set_justify(gtk4::Justification::Center);
        preview_title.set_halign(gtk4::Align::Center);
        preview_content.append(&preview_title);

        // Subtitle
        let preview_subtitle = gtk4::Label::new(None);
        preview_subtitle.add_css_class("preview-subtitle");
        preview_subtitle.set_wrap(true);
        preview_subtitle.set_justify(gtk4::Justification::Center);
        preview_subtitle.set_halign(gtk4::Align::Center);
        preview_content.append(&preview_subtitle);

        // Separator line
        let preview_sep = gtk4::Separator::new(gtk4::Orientation::Horizontal);
        preview_sep.set_margin_top(8);
        preview_sep.set_margin_bottom(8);
        preview_content.append(&preview_sep);

        // Metadata grid
        let preview_metadata = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        preview_content.append(&preview_metadata);

        // Text preview area (for files)
        let preview_text_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .min_content_height(120)
            .visible(false)
            .build();
        preview_content.append(&preview_text_scroll);

        let preview_text = gtk4::TextView::new();
        preview_text.set_editable(false);
        preview_text.set_cursor_visible(false);
        preview_text.set_wrap_mode(gtk4::WrapMode::Word);
        preview_text.add_css_class("preview-text-view");
        preview_text_scroll.set_child(Some(&preview_text));



        // Empty state label
        let empty_label = gtk4::Label::new(None);
        empty_label.set_markup("<span size='large' foreground='#a6adc8'>No results found</span>");
        empty_label.set_halign(gtk4::Align::Center);
        empty_label.set_valign(gtk4::Align::Center);
        empty_label.set_vexpand(true);
        empty_label.set_visible(false);
        content_box.append(&empty_label);

        // Footer Bar
        let footer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        footer.add_css_class("footer");
        main_box.append(&footer);

        let status_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        status_box.set_valign(gtk4::Align::Center);
        footer.append(&status_box);

        let status_label = gtk4::Label::new(Some("Ready"));
        status_label.add_css_class("footer-status");
        status_box.append(&status_label);

        let settings_btn = gtk4::Button::builder()
            .icon_name("preferences-system-symbolic")
            .tooltip_text("Open Settings")
            .valign(gtk4::Align::Center)
            .build();
        settings_btn.add_css_class("footer-icon-btn");
        status_box.append(&settings_btn);

        let s_clone = settings.clone();
        let s_clone_2 = s_clone.clone();
        let w_clone = window.clone();
        let c_clone = provider.clone();
        let p_panel_clone = preview_panel.clone();
        let entry_clone = entry.clone();
        let search_icon_clone = search_icon.clone();
        let listbox_clone = listbox.clone();
        let flowbox_clone = flowbox.clone();
        let scrolled_window_clone = scrolled_window.clone();
        settings_btn.connect_clicked(move |_| {
            w_clone.hide();
            let s_clone = s_clone.clone();
            let s_clone_2 = s_clone_2.clone();
            let w_clone_call = w_clone.clone();
            let w_clone_inner = w_clone.clone();
            let c_clone = c_clone.clone();
            let p_panel = p_panel_clone.clone();
            let entry_call = entry_clone.clone();
            let search_icon_call = search_icon_clone.clone();
            let listbox_call = listbox_clone.clone();
            let flowbox_call = flowbox_clone.clone();
            let scrolled_window_call = scrolled_window_clone.clone();
            crate::settings::show_settings_window(&w_clone_call, s_clone, move || {
                let s = s_clone_2.borrow();
                w_clone_inner.set_size_request(s.window_width, s.window_height);
                c_clone.load_from_data(&generate_css(s.search_font_size, s.title_font_size, &s.theme, &s.color_scheme, s.window_width));
                p_panel.set_visible(s.file_preview_enabled);

                // Update Grid/List view
                if s.grid_view_enabled {
                    scrolled_window_call.set_child(Some(&flowbox_call));
                } else {
                    scrolled_window_call.set_child(Some(&listbox_call));
                }

                if s.layout_mode == "focused" || s.layout_mode == "minimal" {
                    search_icon_call.set_visible(false);
                } else {
                    search_icon_call.set_visible(true);
                }
                if s.layout_mode == "focused" {
                    entry_call.add_css_class("focused-mode");
                } else {
                    entry_call.remove_css_class("focused-mode");
                }

                if s.layout_mode == "minimal" {
                    listbox_call.add_css_class("minimal-mode");
                } else {
                    listbox_call.remove_css_class("minimal-mode");
                }
            });
        });

        let actions_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
        actions_box.set_halign(gtk4::Align::End);
        actions_box.set_hexpand(true);
        footer.append(&actions_box);

        let action_desc = gtk4::Label::new(Some("Actions"));
        action_desc.add_css_class("footer-action-label");
        actions_box.append(&action_desc);

        let action_btn = gtk4::Button::with_label("⌥ Enter");
        action_btn.add_css_class("footer-action-btn");
        actions_box.append(&action_btn);

        // Popover Action Menu
        let popover = gtk4::Popover::new();
        popover.set_parent(&action_btn);
        let popover_list = gtk4::ListBox::new();
        popover.set_child(Some(&popover_list));

        // Setup Async Channel to receive plugin query updates
        let (channel_sender, channel_receiver) = async_channel::unbounded::<(
            String, // plugin_id
            Vec<ResultItem>, // results
            Option<String>, // error
        )>();

        let frecency_store = Rc::new(RefCell::new(crate::frecency::FrecencyStore::load()));

        let state = Rc::new(RefCell::new(WindowState {
            window,
            entry,
            search_icon,
            listbox,
            flowbox,
            scrolled_window,
            empty_label,
            status_label,
            action_btn,
            popover,
            popover_list,
            spinner,
            plugin_manager,
            results_by_plugin: HashMap::new(),
            current_items: Vec::new(),
            channel_sender,
            settings,
            frecency_store,
            css_provider: provider,
            preview_panel,
            preview_icon,
            preview_title,
            preview_subtitle,
            preview_metadata,
            preview_text_scroll,
            preview_text,
            preview_picture,
        }));

        let window_self = Self { state };

        // Connect events
        window_self.setup_event_handlers(channel_receiver);

        window_self
    }

    pub fn toggle_visibility(&self) {
        let is_visible = self.state.borrow().window.is_visible();
        if is_visible {
            let state = self.state.borrow();
            state.popover.popdown();
            state.window.hide();
        } else {
            // Reload apps cache when showing launcher to keep it fresh
            {
                let state = self.state.borrow();
                for p in &state.plugin_manager.internal_plugins {
                    p.reload_cache();
                }
            }

            // Obtain clones of the widgets and release the RefCell borrow
            let (window, entry) = {
                let state = self.state.borrow();
                (state.window.clone(), state.entry.clone())
            };

            window.present();
            entry.set_text("");
            entry.grab_focus();

            // Centre the window on the primary monitor.
            // On Wayland the compositor controls position, but we can nudge it
            // by setting the default size and letting GNOME Shell centre it.
            // We re-apply set_size_request here to make sure it's honoured
            // after any settings change that happened while hidden.
            {
                let state = self.state.borrow();
                let w = state.settings.borrow().window_width;
                let h = state.settings.borrow().window_height;
                state.window.set_size_request(w, h);
            }

            self.update_results();
        }
    }

    fn setup_event_handlers(
        &self,
        channel_receiver: async_channel::Receiver<(String, Vec<ResultItem>, Option<String>)>,
    ) {
        let state_clone = self.state.clone();
        
        // Listen for results on channel receiver (runs on main thread context)
        glib::MainContext::default().spawn_local(async move {
            while let Ok((plugin_id, results, error)) = channel_receiver.recv().await {
                let mut state = state_clone.borrow_mut();
                if let Some(err) = error {
                    println!("Plugin '{}' error: {}", plugin_id, err);
                } else {
                    state.results_by_plugin.insert(plugin_id, results);
                }
                state.spinner.stop();

                // Flatten results
                let mut flat_items = Vec::new();
                for items in state.results_by_plugin.values() {
                    flat_items.extend(items.clone());
                }

                // Inject virtual Settings result if query matches
                let query = state.entry.text().to_string().trim().to_lowercase();
                if !query.is_empty() && (
                    "settings".starts_with(&query) ||
                    "preferences".starts_with(&query) ||
                    "config".starts_with(&query) ||
                    "spear settings".contains(&query)
                ) {
                    flat_items.push(ResultItem {
                        id: "spear-settings".to_string(),
                        title: "Spear Settings".to_string(),
                        subtitle: Some("Configure layout, font sizes, color scheme, and global hotkeys".to_string()),
                        icon: Some("preferences-system-symbolic".to_string()),
                        category: "System".to_string(),
                        score: 200,
                        actions: vec![crate::plugins::Action {
                            label: "Open Settings".to_string(),
                            action_type: "open-settings".to_string(),
                            value: "".to_string(),
                        }],
                    });
                }

                // Apply frecency boost to flat_items scores
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                {
                    let f_store = state.frecency_store.borrow();
                    for item in &mut flat_items {
                        let boost = f_store.get_frecency_score(&item.id, now);
                        item.score += boost;
                    }
                }

                // Sort by score descending
                flat_items.sort_by(|a, b| b.score.cmp(&a.score));
                state.current_items = flat_items;

                // Clone everything we need to update the UI without holding the borrow
                let listbox = state.listbox.clone();
                let flowbox = state.flowbox.clone();
                let empty_label = state.empty_label.clone();
                let scrolled_window = state.scrolled_window.clone();
                let status_label = state.status_label.clone();
                let action_btn = state.action_btn.clone();
                let current_items = state.current_items.clone();
                let query_empty = state.entry.text().trim().is_empty();
                let grid_view_enabled = state.settings.borrow().grid_view_enabled;
                let layout_mode = state.settings.borrow().layout_mode.clone();
                
                // Drop the borrow before we start mutating GtkListBox rows!
                drop(state);

                // 1. Clear containers
                while let Some(row) = listbox.row_at_index(0) {
                    listbox.remove(&row);
                }
                while let Some(child) = flowbox.child_at_index(0) {
                    flowbox.remove(&child);
                }

                if current_items.is_empty() {
                    empty_label.set_visible(!query_empty);
                    scrolled_window.set_visible(query_empty);
                    status_label.set_text("No results");
                    action_btn.set_sensitive(false);
                    
                    // Re-borrow state briefly to clear preview pane
                    let state_ref = state_clone.borrow();
                    clear_preview_pane(&state_ref);
                    continue;
                }

                empty_label.set_visible(false);
                scrolled_window.set_visible(true);

                // 2. Add items
                 if grid_view_enabled {
                     for item in &current_items {
                         let child = build_grid_item(item);
                         flowbox.insert(&child, -1);
                     }
                     if let Some(first_child) = flowbox.child_at_index(0) {
                         flowbox.select_child(&first_child);
                     }
                 } else {
                    for item in &current_items {
                        let row = build_row(item, &layout_mode);
                        listbox.append(&row);
                    }
                    if let Some(first_row) = listbox.row_at_index(0) {
                        listbox.select_row(Some(&first_row));
                    }
                }

                let count = current_items.len();
                status_label.set_text(&format!(
                    "{} result{}",
                    count,
                    if count > 1 { "s found" } else { " found" }
                ));
                action_btn.set_sensitive(true);
            }
        });

        // 2. Search Text Changed
        let state_clone = self.state.clone();
        self.state.borrow().entry.connect_changed(move |entry| {
            let query = entry.text().to_string();
            let mut state = state_clone.borrow_mut();
            state.results_by_plugin.clear();

            if query.trim().is_empty() {
                state.spinner.stop();
            } else {
                state.spinner.start();
            }

            let sender = state.channel_sender.clone();
            let settings = state.settings.borrow().clone();
            state.plugin_manager.query(&query, settings, sender);
        });

        // 3. Row activated in main results list
        let state_clone = self.state.clone();
        self.state.borrow().listbox.connect_row_activated(move |_, row| {
            let idx = row.index();
            let state = state_clone.borrow();
            if idx >= 0 && (idx as usize) < state.current_items.len() {
                let item = state.current_items[idx as usize].clone();
                let window = state.window.clone();
                let popover = state.popover.clone();
                let settings = state.settings.clone();
                let css_provider = state.css_provider.clone();
                let preview_panel = state.preview_panel.clone();
                let entry = state.entry.clone();
                let search_icon = state.search_icon.clone();
                let listbox = state.listbox.clone();
                let frecency_store = state.frecency_store.clone();
                
                drop(state);
                
                execute_action(
                    &item,
                    0,
                    &window,
                    &popover,
                    &settings,
                    &css_provider,
                    &preview_panel,
                    &entry,
                    &search_icon,
                    &listbox,
                    &frecency_store,
                );
            }
        });

        // 4. Keyboard Controller (Global keys inside the window)
        let controller = gtk4::EventControllerKey::new();
        controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let state_clone = self.state.clone();
        controller.connect_key_pressed(move |_, keyval, _keycode, state| {
            let window_state = state_clone.borrow();
            
            if keyval == gdk4::Key::Escape {
                // Hide window
                drop(window_state);
                let state = state_clone.borrow();
                state.popover.popdown();
                state.window.hide();
                return glib::Propagation::Proceed;
            }

            if keyval == gdk4::Key::Return || keyval == gdk4::Key::KP_Enter {
                let alt_pressed = state.contains(gdk4::ModifierType::ALT_MASK);
                let ctrl_pressed = state.contains(gdk4::ModifierType::CONTROL_MASK);

                if alt_pressed || ctrl_pressed {
                    // Show actions popover
                    drop(window_state);
                    show_actions_popover(&state_clone);
                } else {
                    // Trigger first action
                    let selected_idx = if window_state.settings.borrow().grid_view_enabled {
                        window_state.flowbox.selected_children().first().map(|c| c.index())
                    } else {
                        window_state.listbox.selected_row().map(|r| r.index())
                    };

                    if let Some(idx) = selected_idx {
                        if idx >= 0 && (idx as usize) < window_state.current_items.len() {
                            let item = window_state.current_items[idx as usize].clone();
                            let window = window_state.window.clone();
                            let popover = window_state.popover.clone();
                            let settings = window_state.settings.clone();
                            let css_provider = window_state.css_provider.clone();
                            let preview_panel = window_state.preview_panel.clone();
                            let entry = window_state.entry.clone();
                            let search_icon = window_state.search_icon.clone();
                            let listbox = window_state.listbox.clone();
                            let frecency_store = window_state.frecency_store.clone();
                            
                            drop(window_state);
                            
                            execute_action(
                                &item,
                                0,
                                &window,
                                &popover,
                                &settings,
                                &css_provider,
                                &preview_panel,
                                &entry,
                                &search_icon,
                                &listbox,
                                &frecency_store,
                            );
                        }
                    }
                }
                return glib::Propagation::Stop;
            }

            if keyval == gdk4::Key::Down {
                if window_state.settings.borrow().grid_view_enabled {
                    navigate_grid(&window_state.flowbox, &window_state.entry, 4);
                } else {
                    navigate_list(&window_state.listbox, &window_state.entry, 1);
                }
                return glib::Propagation::Stop;
            }

            if keyval == gdk4::Key::Up {
                if window_state.settings.borrow().grid_view_enabled {
                    navigate_grid(&window_state.flowbox, &window_state.entry, -4);
                } else {
                    navigate_list(&window_state.listbox, &window_state.entry, -1);
                }
                return glib::Propagation::Stop;
            }

            if keyval == gdk4::Key::Left {
                if window_state.settings.borrow().grid_view_enabled {
                    navigate_grid(&window_state.flowbox, &window_state.entry, -1);
                    return glib::Propagation::Stop;
                }
            }

            if keyval == gdk4::Key::Right {
                if window_state.settings.borrow().grid_view_enabled {
                    navigate_grid(&window_state.flowbox, &window_state.entry, 1);
                    return glib::Propagation::Stop;
                }
            }

            glib::Propagation::Proceed
        });
        self.state.borrow().window.add_controller(controller);

        // 5. Action Button Clicked
        let state_clone = self.state.clone();
        self.state.borrow().action_btn.connect_clicked(move |_| {
            show_actions_popover(&state_clone);
        });

        // 6. Popover Row Activated
        let state_clone = self.state.clone();
        self.state.borrow().popover_list.connect_row_activated(move |_, row| {
            let action_idx = row.index() as usize;
            let state = state_clone.borrow();
            state.popover.popdown();

            if let Some(selected_row) = state.listbox.selected_row() {
                let item_idx = selected_row.index() as usize;
                if item_idx < state.current_items.len() {
                    let item = state.current_items[item_idx].clone();
                    let window = state.window.clone();
                    let popover = state.popover.clone();
                    let settings = state.settings.clone();
                    let css_provider = state.css_provider.clone();
                    let preview_panel = state.preview_panel.clone();
                    let entry = state.entry.clone();
                    let search_icon = state.search_icon.clone();
                    let listbox = state.listbox.clone();
                    let frecency_store = state.frecency_store.clone();
                    
                    drop(state);
                    
                    execute_action(
                        &item,
                        action_idx,
                        &window,
                        &popover,
                        &settings,
                        &css_provider,
                        &preview_panel,
                        &entry,
                        &search_icon,
                        &listbox,
                        &frecency_store,
                    );
                }
            }
        });

        // 7. Hide on focus lost
        let state_clone = self.state.clone();
        self.state.borrow().window.connect_notify_local(Some("is-active"), move |window, _| {
            if !window.is_active() {
                let state = state_clone.borrow();
                state.popover.popdown();
                state.window.hide();
            }
        });

        // 8. Row/Child Selected (update preview panel)
        let state_clone = self.state.clone();
        let listbox = self.state.borrow().listbox.clone();
        listbox.connect_row_selected(move |_, row_opt| {
            let state = state_clone.borrow();
            if !state.settings.borrow().file_preview_enabled {
                state.preview_panel.set_visible(false);
                return;
            }

            if let Some(row) = row_opt {
                let idx = row.index();
                if idx >= 0 && (idx as usize) < state.current_items.len() {
                    let item = &state.current_items[idx as usize];
                    update_preview_pane(item, &state);
                }
            } else {
                clear_preview_pane(&state);
            }
        });

        let state_clone = self.state.clone();
        let flowbox = self.state.borrow().flowbox.clone();
        flowbox.connect_child_activated(move |_, child| {
            let idx = child.index();
            let state = state_clone.borrow();
            if idx >= 0 && (idx as usize) < state.current_items.len() {
                let item = state.current_items[idx as usize].clone();
                let window = state.window.clone();
                let popover = state.popover.clone();
                let settings = state.settings.clone();
                let css_provider = state.css_provider.clone();
                let preview_panel = state.preview_panel.clone();
                let entry = state.entry.clone();
                let search_icon = state.search_icon.clone();
                let listbox = state.listbox.clone();
                let frecency_store = state.frecency_store.clone();
                
                drop(state);
                
                execute_action(
                    &item,
                    0,
                    &window,
                    &popover,
                    &settings,
                    &css_provider,
                    &preview_panel,
                    &entry,
                    &search_icon,
                    &listbox,
                    &frecency_store,
                );
            }
        });

        let state_clone = self.state.clone();
        flowbox.connect_selected_children_changed(move |fb| {
            let state = state_clone.borrow();
            if !state.settings.borrow().file_preview_enabled {
                state.preview_panel.set_visible(false);
                return;
            }

            if let Some(child) = fb.selected_children().first() {
                let idx = child.index();
                if idx >= 0 && (idx as usize) < state.current_items.len() {
                    let item = &state.current_items[idx as usize];
                    update_preview_pane(item, &state);
                }
            } else {
                clear_preview_pane(&state);
            }
        });
    }

    fn update_results(&self) {
        let state = self.state.borrow();
        let sender = state.channel_sender.clone();
        let query = state.entry.text().to_string();
        let settings = state.settings.borrow().clone();
        state.plugin_manager.query(&query, settings, sender);
    }
}

fn build_row(item: &ResultItem, layout_mode: &str) -> gtk4::ListBoxRow {
    let row = gtk4::ListBoxRow::new();
    row.set_focus_on_click(false);

    let box_layout = gtk4::Box::new(gtk4::Orientation::Horizontal, 14);
    box_layout.set_margin_start(6);
    box_layout.set_margin_end(6);
    box_layout.set_margin_top(6);
    box_layout.set_margin_bottom(6);
    // Don't let the row request more width than the container
    box_layout.set_hexpand(false);
    row.set_child(Some(&box_layout));

    if layout_mode != "minimal" {
        // Icon resolution:
        //  1. Strip "-symbolic" suffix so GTK looks up the full-colour Adwaita
        //     icon first (e.g. "folder" instead of "folder-symbolic"). The theme
        //     automatically falls back to the symbolic variant if no colour icon exists.
        //  2. Absolute paths that are SVGs → treat the stem as a named icon so
        //     the Adwaita theme renderer applies colours.
        //  3. Absolute paths that are bitmaps → render directly via GFileIcon.
        let icon_str = item.icon.as_deref().unwrap_or("system-run");

        // Resolve path for custom icon
        let mut custom_path = None;
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/atharva".to_string());
        let installed_path = std::path::PathBuf::from(home)
            .join(".config")
            .join("spear")
            .join("icons")
            .join(icon_str);
        let system_path = std::path::PathBuf::from("/usr/share/spear/icons").join(icon_str);
        if installed_path.exists() && installed_path.is_file() {
            custom_path = Some(installed_path);
        } else if system_path.exists() && system_path.is_file() {
            custom_path = Some(system_path);
        } else {
            // Fallback to local icons/ directory relative to current working directory (for development)
            let local_path = std::path::PathBuf::from("icons").join(icon_str);
            if local_path.exists() && local_path.is_file() {
                custom_path = Some(local_path);
            }
        }

        let image = if let Some(path) = custom_path {
            // Custom downloaded SVG/PNG icon
            let file = gio::File::for_path(&path);
            gtk4::Image::from_gicon(&gio::FileIcon::new(&file))
        } else if icon_str.starts_with('/') && std::path::Path::new(icon_str).exists() {
            let path = std::path::Path::new(icon_str);
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "svg" {
                // Use the filename stem as a named icon (e.g. "folder" from "folder.svg")
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("application-x-executable");
                let name = stem.trim_end_matches("-symbolic");
                
                let resolved_name = if let Some(display) = gdk4::Display::default() {
                    let icon_theme = gtk4::IconTheme::for_display(&display);
                    if icon_theme.has_icon(name) {
                        name.to_string()
                    } else {
                        stem.to_string()
                    }
                } else {
                    name.to_string()
                };
                gtk4::Image::from_icon_name(&resolved_name)
            } else {
                // Bitmap thumbnail — display as-is
                let file = gio::File::for_path(icon_str);
                gtk4::Image::from_gicon(&gio::FileIcon::new(&file))
            }
        } else {
            // Named icon — strip "-symbolic" to prefer the colour version ONLY if the colour version exists
            let color_name = icon_str.trim_end_matches("-symbolic");
            let resolved_name = if let Some(display) = gdk4::Display::default() {
                let icon_theme = gtk4::IconTheme::for_display(&display);
                if icon_theme.has_icon(color_name) {
                    color_name.to_string()
                } else {
                    icon_str.to_string()
                }
            } else {
                color_name.to_string()
            };
            gtk4::Image::from_icon_name(&resolved_name)
        };

        image.set_pixel_size(32);
        image.set_valign(gtk4::Align::Center);
        box_layout.append(&image);
    }

    // Text Box — hexpand so it absorbs available space and prevents
    // the category badge from pushing the row wider than the window
    let text_box = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    text_box.set_valign(gtk4::Align::Center);
    text_box.set_hexpand(true);
    // Clamp labels so they never request more width than available
    text_box.set_overflow(gtk4::Overflow::Hidden);
    box_layout.append(&text_box);

    let title_label = gtk4::Label::new(Some(&item.title));
    title_label.set_halign(gtk4::Align::Start);
    title_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    title_label.set_max_width_chars(40);
    title_label.add_css_class("row-title");
    text_box.append(&title_label);

    if layout_mode != "minimal" {
        if let Some(ref subtitle) = item.subtitle {
            let subtitle_label = gtk4::Label::new(Some(subtitle));
            subtitle_label.set_halign(gtk4::Align::Start);
            subtitle_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            subtitle_label.set_max_width_chars(50);
            subtitle_label.add_css_class("row-subtitle");
            text_box.append(&subtitle_label);
        }
    }

    // Category badge — fixed on the right; does NOT hexpand
    let cat_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    cat_box.set_halign(gtk4::Align::End);
    cat_box.set_valign(gtk4::Align::Center);
    cat_box.set_hexpand(false);
    box_layout.append(&cat_box);

    let cat_label = gtk4::Label::new(None);
    cat_label.set_markup(&format!(
        "<span size='x-small'>{}</span>",
        glib::markup_escape_text(&item.category)
    ));
    cat_label.add_css_class("row-category");
    cat_box.append(&cat_label);

    row
}

fn navigate_list(listbox: &gtk4::ListBox, entry: &gtk4::Entry, direction: i32) {
    if let Some(selected_row) = listbox.selected_row() {
        let idx = selected_row.index();
        if let Some(next_row) = listbox.row_at_index(idx + direction) {
            listbox.select_row(Some(&next_row));
            next_row.grab_focus();
            entry.grab_focus();
        }
    } else if let Some(first_row) = listbox.row_at_index(0) {
        listbox.select_row(Some(&first_row));
    }
}

fn navigate_grid(flowbox: &gtk4::FlowBox, entry: &gtk4::Entry, direction: i32) {
    let children = flowbox.selected_children();
    if let Some(selected) = children.first() {
        let idx = selected.index();
        if let Some(next) = flowbox.child_at_index(idx + direction) {
            flowbox.select_child(&next);
            next.grab_focus();
            entry.grab_focus();
        }
    } else if let Some(first) = flowbox.child_at_index(0) {
        flowbox.select_child(&first);
    }
}

fn build_grid_item(item: &ResultItem) -> gtk4::FlowBoxChild {
    let child = gtk4::FlowBoxChild::new();
    let box_layout = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    box_layout.set_margin_start(10);
    box_layout.set_margin_end(10);
    box_layout.set_margin_top(10);
    box_layout.set_margin_bottom(10);
    child.set_child(Some(&box_layout));

    let icon_str = item.icon.as_deref().unwrap_or("system-run");
    let icon = gtk4::Image::builder()
        .pixel_size(48)
        .halign(gtk4::Align::Center)
        .build();

    if icon_str.starts_with('/') && std::path::Path::new(icon_str).exists() {
        let file = gio::File::for_path(icon_str);
        let file_icon = gio::FileIcon::new(&file);
        icon.set_from_gicon(&file_icon);
    } else {
        icon.set_icon_name(Some(icon_str));
    }
    box_layout.append(&icon);

    let title = gtk4::Label::new(Some(&item.title));
    title.set_halign(gtk4::Align::Center);
    title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    title.set_max_width_chars(12);
    title.add_css_class("grid-title");
    box_layout.append(&title);

    child
}

fn show_actions_popover(state_rc: &Rc<RefCell<WindowState>>) {
    let state = state_rc.borrow();
    let selected_idx = if state.settings.borrow().grid_view_enabled {
        state.flowbox.selected_children().first().map(|c| c.index())
    } else {
        state.listbox.selected_row().map(|r| r.index())
    };

    if selected_idx.is_none() {
        return;
    }

    let item_idx = selected_idx.unwrap() as usize;
    if item_idx >= state.current_items.len() {
        return;
    }

    let item = &state.current_items[item_idx];
    let actions = &item.actions;
    if actions.is_empty() {
        return;
    }

    // Clear popover items
    while let Some(row) = state.popover_list.row_at_index(0) {
        state.popover_list.remove(&row);
    }

    for (idx, action) in actions.iter().enumerate() {
        let row = gtk4::ListBoxRow::new();
        let box_layout = gtk4::Box::new(gtk4::Orientation::Horizontal, 20);
        box_layout.set_margin_start(10);
        box_layout.set_margin_end(10);
        box_layout.set_margin_top(6);
        box_layout.set_margin_bottom(6);
        row.set_child(Some(&box_layout));

        let label = gtk4::Label::new(Some(&action.label));
        label.set_halign(gtk4::Align::Start);
        box_layout.append(&label);

        let shortcut = match idx {
            0 => "↵".to_string(),
            n if n < 9 => format!("⌥{}", n + 1),
            _ => String::new(),
        };

        if !shortcut.is_empty() {
            let sh_label = gtk4::Label::new(None);
            sh_label.set_markup(&format!(
                "<span foreground='#a6adc8' size='small'>{}</span>",
                shortcut
            ));
            sh_label.set_halign(gtk4::Align::End);
            sh_label.set_hexpand(true);
            box_layout.append(&sh_label);
        }

        state.popover_list.append(&row);
    }

    state.popover.popup();
}

fn execute_action(
    item: &ResultItem,
    index: usize,
    window: &libadwaita::ApplicationWindow,
    popover: &gtk4::Popover,
    settings: &Rc<RefCell<AppSettings>>,
    css_provider: &gtk4::CssProvider,
    preview_panel: &gtk4::Box,
    entry: &gtk4::Entry,
    search_icon: &gtk4::Image,
    listbox: &gtk4::ListBox,
    frecency_store: &Rc<RefCell<crate::frecency::FrecencyStore>>,
) {
    // Record execution for frecency
    frecency_store.borrow_mut().record_access(&item.id);

    if index >= item.actions.len() {
        return;
    }

    let action = &item.actions[index];
    popover.popdown();

    match action.action_type.as_str() {
        "open-settings" => {
            window.hide();
            let s_clone = settings.clone();
            let s_clone_2 = s_clone.clone();
            let w_clone = window.clone();
            let c_clone = css_provider.clone();
            let p_panel = preview_panel.clone();
            let entry_clone = entry.clone();
            let search_icon_clone = search_icon.clone();
            let listbox_clone = listbox.clone();
            crate::settings::show_settings_window(window, s_clone, move || {
                let s = s_clone_2.borrow();
                // 1. Resize main window to new settings width
                w_clone.set_size_request(s.window_width, s.window_height);
                // 2. Reload custom CSS styles
                c_clone.load_from_data(&generate_css(s.search_font_size, s.title_font_size, &s.theme, &s.color_scheme, s.window_width));
                p_panel.set_visible(s.file_preview_enabled);

                // Update layout mode dynamically
                if s.layout_mode == "focused" || s.layout_mode == "minimal" {
                    search_icon_clone.set_visible(false);
                } else {
                    search_icon_clone.set_visible(true);
                }
                if s.layout_mode == "focused" {
                    entry_clone.add_css_class("focused-mode");
                } else {
                    entry_clone.remove_css_class("focused-mode");
                }

                if s.layout_mode == "minimal" {
                    listbox_clone.add_css_class("minimal-mode");
                } else {
                    listbox_clone.remove_css_class("minimal-mode");
                }
            });
        }
        "launch-app" => {
            window.hide();
            if let Some(app_info) = gio::DesktopAppInfo::new(&action.value) {
                let _ = app_info.launch(&[], gio::AppLaunchContext::NONE);
            }
        }
        "open-url" => {
            window.hide();
            let _ = launch_url(&action.value);
        }
        "copy-to-clipboard" => {
            copy_to_clipboard(&action.value);
            window.hide();
        }
        "run-command" => {
            window.hide();
            let _ = std::process::Command::new("sh")
                .arg("-c")
                .arg(&action.value)
                .spawn();
        }
        "navigate-dir" => {
            entry.set_text(&action.value);
            entry.set_position(-1);
        }
        _ => {}
    }
}

fn update_preview_pane(item: &ResultItem, state: &WindowState) {
    state.preview_panel.set_visible(true);

    // Set Title and Subtitle
    state.preview_title.set_text(&item.title);
    state.preview_subtitle.set_text(item.subtitle.as_deref().unwrap_or(""));

    // Set Icon
    let icon_str = item.icon.as_deref().unwrap_or("system-run-symbolic");
    if icon_str.starts_with('/') && std::path::Path::new(icon_str).exists() {
        let file = gio::File::for_path(icon_str);
        let file_icon = gio::FileIcon::new(&file);
        state.preview_icon.set_from_gicon(&file_icon);
    } else {
        state.preview_icon.set_icon_name(Some(icon_str));
    }

    // Clear metadata box
    while let Some(child) = state.preview_metadata.first_child() {
        state.preview_metadata.remove(&child);
    }

    // Add general metadata
    add_metadata_row(&state.preview_metadata, "Category", &item.category);

    // Detect file path
    let mut file_path: Option<std::path::PathBuf> = None;
    if item.category == "Applications" {
        if let Some(action) = item.actions.first() {
            if let Some(app_info) = gio::DesktopAppInfo::new(&action.value) {
                if let Some(filename) = app_info.filename() {
                    file_path = Some(filename);
                    if let Some(cmd) = app_info.commandline() {
                        add_metadata_row(&state.preview_metadata, "Command", &cmd.to_string_lossy());
                    }
                }
            }
        }
    } else if std::path::Path::new(&item.title).exists() {
        file_path = Some(std::path::PathBuf::from(&item.title));
    } else if let Some(ref subtitle) = item.subtitle {
        if std::path::Path::new(subtitle).exists() {
            file_path = Some(std::path::PathBuf::from(subtitle));
        }
    }

    if file_path.is_none() {
        for action in &item.actions {
            if std::path::Path::new(&action.value).exists() {
                file_path = Some(std::path::PathBuf::from(&action.value));
                break;
            }
        }
    }

    // Clear and hide text/image previews by default, show fallback icon by default
    state.preview_picture.set_visible(false);
    state.preview_icon.set_visible(true);
    state.preview_text.buffer().set_text("");
    state.preview_text_scroll.set_visible(false);

    if let Some(path) = file_path {
        add_metadata_row(&state.preview_metadata, "Path", &path.to_string_lossy());
        
        if let Ok(metadata) = std::fs::metadata(&path) {
            let file_type = if metadata.is_dir() {
                "Directory"
            } else if metadata.is_symlink() {
                "Symlink"
            } else {
                "File"
            };
            add_metadata_row(&state.preview_metadata, "Type", file_type);

            if metadata.is_file() {
                add_metadata_row(&state.preview_metadata, "Size", &format_size(metadata.len()));
            }

            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                    if let Ok(dt) = glib::DateTime::from_unix_local(duration.as_secs() as i64) {
                        if let Ok(formatted) = dt.format("%Y-%m-%d %H:%M") {
                            add_metadata_row(&state.preview_metadata, "Modified", &formatted);
                        }
                    }
                }
            }

            // Try to resolve from Nautilus thumbnail cache
            let mut thumbnail_loaded = false;
            
            if metadata.is_file() {
                if let Some(thumb_path) = get_cached_thumbnail(&path) {
                    let file = gio::File::for_path(&thumb_path);
                    state.preview_picture.set_file(Some(&file));
                    state.preview_picture.set_visible(true);
                    state.preview_icon.set_visible(false); // Hide fallback icon
                    thumbnail_loaded = true;
                }
            }

            if !thumbnail_loaded {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                let is_image = ["png", "jpg", "jpeg", "gif", "svg", "webp", "bmp", "ico"].contains(&ext.as_str());

                if metadata.is_file() && is_image {
                    let file = gio::File::for_path(&path);
                    state.preview_picture.set_file(Some(&file));
                    state.preview_picture.set_visible(true);
                    state.preview_icon.set_visible(false); // Hide fallback icon
                } else if metadata.is_file() && metadata.len() < 10 * 1024 * 1024 {
                    // Show text preview for files under 10MB
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let preview_limit = 500;
                        let preview_str: String = content.chars().take(preview_limit).collect();
                        state.preview_text.buffer().set_text(&preview_str);
                        state.preview_text_scroll.set_visible(true);
                    }
                }
            }
        }
    } else {
        // Not a file, show specific metadata
        if item.category == "Calculator" {
            if let Some(ref subtitle) = item.subtitle {
                add_metadata_row(&state.preview_metadata, "Expression", subtitle);
            }
            state.preview_text.buffer().set_text(&format!(
                "\n\n    Result:\n\n    {}\n\n    (Press Enter to copy to clipboard)",
                item.title
            ));
            state.preview_text_scroll.set_visible(true);
        } else if item.category == "Web Search" || item.category == "YouTube Search" {
            if let Some(action) = item.actions.first() {
                add_metadata_row(&state.preview_metadata, "URL", &action.value);
                state.preview_text.buffer().set_text(&format!(
                    "\n\n    Action: {}\n\n    Link: {}\n\n    (Press Enter to open in browser)",
                    action.label, action.value
                ));
                state.preview_text_scroll.set_visible(true);
            }
        } else if item.category == "Run Command" {
            if let Some(action) = item.actions.first() {
                add_metadata_row(&state.preview_metadata, "Command", &action.value);
                state.preview_text.buffer().set_text(&format!(
                    "\n\n    Execute in background subshell:\n\n    $ {}\n\n    (Press Enter to run)",
                    action.value
                ));
                state.preview_text_scroll.set_visible(true);
            }
        } else if item.category == "GNOME Controls" {
            if let Some(action) = item.actions.first() {
                add_metadata_row(&state.preview_metadata, "Command", &action.value);
                state.preview_text.buffer().set_text(&format!(
                    "\n\n    GNOME System Action:\n\n    {}\n\n    (Press Enter to execute)",
                    item.title
                ));
                state.preview_text_scroll.set_visible(true);
            }
        }
    }
}

fn clear_preview_pane(state: &WindowState) {
    state.preview_panel.set_visible(false);
}

fn add_metadata_row(container: &gtk4::Box, label: &str, value: &str) {
    let row_box = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    row_box.set_margin_bottom(8);

    let lbl = gtk4::Label::new(Some(label));
    lbl.add_css_class("preview-metadata-label");
    lbl.set_halign(gtk4::Align::Start);
    row_box.append(&lbl);

    let val = gtk4::Label::new(Some(value));
    val.add_css_class("preview-metadata-value");
    val.set_halign(gtk4::Align::Start);
    val.set_wrap(true);
    row_box.append(&val);

    container.append(&row_box);
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn get_cached_thumbnail(file_path: &std::path::Path) -> Option<std::path::PathBuf> {
    let file = gio::File::for_path(file_path);
    let uri = file.uri();
    
    let md5_hex = glib::compute_checksum_for_string(glib::ChecksumType::Md5, &uri)?;
    
    let home = std::env::var("HOME").ok()?;
    let cache_dir = std::path::PathBuf::from(home).join(".cache").join("thumbnails");
    
    let large_path = cache_dir.join("large").join(format!("{}.png", md5_hex));
    if large_path.exists() && large_path.is_file() {
        return Some(large_path);
    }
    
    let normal_path = cache_dir.join("normal").join(format!("{}.png", md5_hex));
    if normal_path.exists() && normal_path.is_file() {
        return Some(normal_path);
    }
    
    None
}
