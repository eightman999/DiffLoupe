//! diffloupe-gtk: DiffLoupe Linux 版(GTK4)のライブラリ。
//!
//! Free の bin(`src/main.rs`)はこのクレートの [`run`] を呼ぶだけ。Pro 版(別 repo)は
//! [`AppConfig`] にアプリ ID・表示名・エディション・GTK 拡張・設定保存先を渡して同じ
//! 実装を再利用する(設計: `docs/RUST_EDITION.md`)。

pub mod css;
pub mod detail;
pub mod edition_ext;
pub mod hexview;
pub mod imageops;
pub mod imageview;
pub mod model;
pub mod recent;
pub mod settings;
pub mod settingsui;
pub mod smoke;
pub mod strings;
pub mod syncplan;
pub mod textdiff;
pub mod threeway;
pub mod tree;
pub mod window;

use std::process::ExitCode;
use std::rc::Rc;

use diffloupe_edition::Edition;
use gtk4::prelude::*;

pub use edition_ext::{FreeGtkEdition, GtkEditionExtension};
pub use window::MainWindow;

/// ウィンドウ表示後に 1 回呼ばれるフック(スモークテストの駆動などに使う)。
pub type ReadyHook = Box<dyn Fn(&Rc<MainWindow>)>;

/// アプリの構成。Free は [`AppConfig::free`]、Pro は各フィールドを差し替えて [`run`] に渡す。
pub struct AppConfig {
    /// GApplication の ID(Free と Pro で別にして同時起動・同時インストールできるようにする)
    pub app_id: String,
    /// ウィンドウタイトルに出す表示名("DiffLoupe" / "DiffLoupe Pro")
    pub app_name: String,
    /// 設定・履歴の保存先ディレクトリ名(`$XDG_CONFIG_HOME/<名前>/`)
    pub config_dir_name: String,
    /// エディション(名前・件数上限・未保存フック・能力フラグ)
    pub edition: Box<dyn Edition>,
    /// GTK ビュー差し替え・メニュー/アクション追加の拡張
    pub extension: Box<dyn GtkEditionExtension>,
    /// ウィンドウ表示後のフック
    pub on_ready: Option<ReadyHook>,
    /// 追加 CSS(既定の CSS の後に適用)
    pub extra_css: Option<String>,
}

impl AppConfig {
    /// 公開版(Free)の構成。
    pub fn free() -> Self {
        Self {
            app_id: "com.furin-lab.diffloupe".to_string(),
            app_name: strings::APP_NAME.to_string(),
            config_dir_name: "diffloupe".to_string(),
            edition: Box::new(diffloupe_edition::FreeEdition),
            extension: Box::new(FreeGtkEdition),
            on_ready: None,
            extra_css: None,
        }
    }
}

/// アプリを起動して終了コードを返す(`Application::run` 相当)。
pub fn run(config: AppConfig) -> ExitCode {
    let AppConfig {
        app_id,
        app_name,
        config_dir_name,
        edition,
        extension,
        on_ready,
        extra_css,
    } = config;
    settings::set_config_dir_name(&config_dir_name);

    let app = gtk4::Application::new(
        Some(app_id.as_str()),
        gtk4::gio::ApplicationFlags::NON_UNIQUE,
    );
    let edition: Rc<dyn Edition> = Rc::from(edition);
    let extension: Rc<dyn GtkEditionExtension> = Rc::from(extension);
    app.connect_activate(move |app| {
        install_css(extra_css.as_deref());
        let w = MainWindow::new(app, edition.clone(), extension.clone(), &app_name);
        w.window.present();
        if let Some(hook) = &on_ready {
            hook(&w);
        }
    });
    let code = app.run_with_args::<&str>(&[]);
    ExitCode::from(code.value() as u8)
}

fn install_css(extra: Option<&str>) {
    let Some(display) = gtk4::gdk::Display::default() else {
        return;
    };
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(css::APP_CSS);
    gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    if let Some(extra) = extra {
        let p2 = gtk4::CssProvider::new();
        p2.load_from_data(extra);
        gtk4::style_context_add_provider_for_display(
            &display,
            &p2,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Free の ID・設定先・件数上限はライブラリ化の前と同じ(同時インストール・設定の互換)。
    #[test]
    fn free_config_keeps_the_previous_identity() {
        let c = AppConfig::free();
        assert_eq!(c.app_id, "com.furin-lab.diffloupe");
        assert_eq!(c.app_name, "DiffLoupe");
        assert_eq!(c.config_dir_name, "diffloupe");
        assert_eq!(c.edition.name(), "Free");
        assert_eq!(c.edition.max_folder_entries(), Some(2000));
        assert!(c.edition.confirm_discard_unsaved_changes());
        assert!(!c.edition.has_unsaved_changes());
        assert!(c.on_ready.is_none() && c.extra_css.is_none());
    }

    #[test]
    fn default_config_dir_is_diffloupe() {
        // set_config_dir_name を呼ぶ前(このテストプロセスでは呼ばない)の既定
        assert!(settings::config_dir().ends_with("diffloupe"));
    }
}
