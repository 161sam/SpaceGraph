use bevy::prelude::*;
use bevy_egui::EguiPlugin;

use spacegraph_viewer::app::resources::{NetRx, NetTx};
use spacegraph_viewer::app::SpaceGraphViewerPlugin;

fn main() {
    let demo_load = parse_demo_load(std::env::args().skip(1));

    let (tx, rx) = crossbeam_channel::unbounded();

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "SpaceGraph (native)".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin)
        .insert_resource(NetRx(rx))
        .insert_resource(NetTx(tx))
        .add_plugins(SpaceGraphViewerPlugin { demo_load })
        .run();
}

/// Parse `--demo-load <n>` (or `--demo-load=<n>`) from the CLI arguments.
///
/// Seeds the viewer with a deterministic synthetic graph of `n` nodes for
/// benchmarking and visual smoke testing instead of connecting to an agent.
fn parse_demo_load<I: Iterator<Item = String>>(mut args: I) -> Option<usize> {
    while let Some(arg) = args.next() {
        if let Some(rest) = arg.strip_prefix("--demo-load=") {
            return rest.parse().ok();
        }
        if arg == "--demo-load" {
            return args.next().and_then(|v| v.parse().ok());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::parse_demo_load;

    fn parse(args: &[&str]) -> Option<usize> {
        parse_demo_load(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn parses_space_separated_flag() {
        assert_eq!(parse(&["--demo-load", "2000"]), Some(2000));
    }

    #[test]
    fn parses_equals_flag() {
        assert_eq!(parse(&["--demo-load=5000"]), Some(5000));
    }

    #[test]
    fn absent_flag_is_none() {
        assert_eq!(parse(&["--other", "1"]), None);
        assert_eq!(parse(&[]), None);
    }
}
