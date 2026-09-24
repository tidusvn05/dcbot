use anyhow::{bail, Result};
use console::style;
use rust_i18n::t;

use crate::config;

pub fn set_lang(lang: &str) -> Result<()> {
    if !config::VALID_LANGS.contains(&lang) {
        bail!(t!(
            "config.bad_lang",
            langs = config::VALID_LANGS.join(", ")
        ));
    }
    config::set_lang(lang)?;
    rust_i18n::set_locale(lang);
    println!(
        "{} {}",
        style("✓").green().bold(),
        t!("config.lang_set", lang = lang)
    );
    Ok(())
}

pub fn get() -> Result<()> {
    let cfg = config::load();
    println!("{} {}", t!("config.file"), config::config_file().display());
    println!("lang = {}", cfg.lang.as_deref().unwrap_or("(auto)"));
    println!(
        "{}",
        t!(
            "config.effective",
            lang = config::resolve_lang(None).as_str()
        )
    );
    Ok(())
}
