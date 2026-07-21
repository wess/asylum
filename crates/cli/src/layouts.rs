//! `asylum layout` - inspect the fan-out presets defined in settings.json.

use crate::help;
use crate::positionals;

pub fn layout(args: &[String]) -> Result<(), String> {
    let settings = config::load(&config::default_path()).settings;
    match args.first().map(String::as_str).unwrap_or("list") {
        "list" => {
            if settings.layouts.is_empty() {
                println!("no layouts defined");
            }
            for l in &settings.layouts {
                println!(
                    "{:<10} {:<38} [{}]",
                    l.name,
                    l.description,
                    l.agents.join(", ")
                );
            }
            Ok(())
        }
        "show" => {
            let name = positionals(&args[1..]).first().cloned().ok_or_else(|| {
                format!(
                    "usage: asylum layout show <name> {}",
                    help::hint(&["layout", "show"])
                )
            })?;
            let l = settings
                .layout(&name)
                .ok_or_else(|| format!("no layout `{name}`"))?;
            println!("name:        {}", l.name);
            println!("description: {}", l.description);
            println!("agents:      {}", l.agents.join(", "));
            println!(
                "concurrency: {}",
                if l.concurrency == 0 {
                    "all at once".to_string()
                } else {
                    l.concurrency.to_string()
                }
            );
            Ok(())
        }
        _ => Err(format!(
            "usage: asylum layout <list | show <name>> {}",
            help::hint(&["layout"])
        )),
    }
}
