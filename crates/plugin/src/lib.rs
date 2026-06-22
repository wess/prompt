//! Manifest-based plugins for Prompt.
//!
//! A plugin is a directory containing `plugin.toml`. The first extension
//! point is command contributions: plugins can expose shell commands, give
//! them titles, and optionally assign default keybindings.

mod load;
mod manifest;

pub use load::{defaultdir, load};
pub use manifest::{parse, Command, CommandMode, Diagnostic, Plugin};

/// The manifest filename inside a plugin directory.
pub const MANIFEST: &str = "plugin.toml";

/// Stable action id for a contributed command.
pub fn actionid(plugin: &str, command: &str) -> String {
    format!("{plugin}/{command}")
}

/// Convert plugin command keybindings into config keybind entries. These
/// are intentionally ordinary action strings so user config can override
/// or unbind them with the existing resolver.
pub fn keybinds(plugins: &[Plugin]) -> Vec<String> {
    let mut binds = Vec::new();
    for plugin in plugins {
        for command in &plugin.commands {
            let Some(keybind) = command.keybind.as_ref() else {
                continue;
            };
            binds.push(format!(
                "{keybind}=plugin_command:{}",
                actionid(&plugin.id, &command.id)
            ));
        }
    }
    binds
}

/// Find a command by the action id returned from [`actionid`].
pub fn command<'a>(plugins: &'a [Plugin], id: &str) -> Option<(&'a Plugin, &'a Command)> {
    let (pluginid, commandid) = id.split_once('/')?;
    plugins.iter().find_map(|plugin| {
        (plugin.id == pluginid).then(|| {
            plugin
                .commands
                .iter()
                .find(|command| command.id == commandid)
                .map(|command| (plugin, command))
        })?
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keybind_entries_use_plugin_actions() {
        let plugins = vec![Plugin {
            id: "tools".to_string(),
            name: "Tools".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            path: std::path::PathBuf::from("/tmp/tools"),
            commands: vec![Command {
                id: "top".to_string(),
                title: "Top".to_string(),
                run: "top".to_string(),
                mode: CommandMode::Tab,
                keybind: Some("cmd+shift+t".to_string()),
            }],
        }];
        assert_eq!(
            keybinds(&plugins),
            vec!["cmd+shift+t=plugin_command:tools/top".to_string()]
        );
        assert_eq!(command(&plugins, "tools/top").unwrap().1.run, "top");
        assert!(command(&plugins, "tools/missing").is_none());
    }
}
