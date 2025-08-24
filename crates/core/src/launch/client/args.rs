use mcvm_shared::output::{MCVMOutput, MessageContents, MessageLevel};
use mcvm_shared::util::{ARCH_STRING, OS_STRING};
use mcvm_shared::versions::VersionPattern;

use crate::instance::{InstanceKind, WindowResolution};
use crate::launch::{LaunchParameters, QuickPlayType};

use crate::io::files::paths::Paths;
use crate::net::game_files::assets::get_virtual_dir_path;
use crate::net::game_files::client_meta::args::ArgumentItem;
use crate::user::UserKind;

/// Process an argument for the client from the client meta
pub(crate) fn process_arg(arg: &ArgumentItem, params: &LaunchParameters) -> Vec<String> {
	let mut out = Vec::new();
	let InstanceKind::Client { window } = &params.side else {
		panic!("Instance is not a client")
	};
	match arg {
		ArgumentItem::Simple(arg) => {
			let arg = process_simple_arg(arg, params);
			if let Some(arg) = arg {
				out.push(arg);
			}
		}
		ArgumentItem::Conditional(arg) => {
			for rule in &arg.rules {
				let allowed = rule.action.is_allowed();

				if let Some(os_name) = &rule.os.name {
					if allowed != (OS_STRING == os_name.to_string()) {
						return vec![];
					}
				}
				if let Some(os_arch) = &rule.os.arch {
					if allowed != (ARCH_STRING == os_arch.to_string()) {
						return vec![];
					}
				}

				if let Some(has_custom_resolution) = &rule.features.has_custom_resolution {
					if *has_custom_resolution && window.resolution.is_none() {
						return vec![];
					}
				}
				if let Some(is_demo_user) = &rule.features.is_demo_user {
					if *is_demo_user {
						let use_demo = match params.users.get_chosen_user() {
							Some(user) => matches!(user.kind, UserKind::Demo),
							None => false,
						};
						if !use_demo {
							return vec![];
						}
					}
				}
				if let Some(quick_play_support) = &rule.features.has_quick_play_support {
					if *quick_play_support {
						let uses_quick_play =
							!matches!(params.launch_config.quick_play, QuickPlayType::None);
						if !uses_quick_play {
							return vec![];
						}
					}
				}
				if let Some(quick_play_singleplayer) = &rule.features.is_quick_play_singleplayer {
					if *quick_play_singleplayer {
						let uses_quick_play =
							!matches!(params.launch_config.quick_play, QuickPlayType::World { .. });
						if !uses_quick_play {
							return vec![];
						}
					}
				}
				if let Some(quick_play_multiplayer) = &rule.features.is_quick_play_multiplayer {
					if *quick_play_multiplayer {
						let uses_quick_play = !matches!(
							params.launch_config.quick_play,
							QuickPlayType::Server { .. }
						);
						if !uses_quick_play {
							return vec![];
						}
					}
				}
				if let Some(quick_play_realms) = &rule.features.is_quick_play_realms {
					if *quick_play_realms {
						let uses_quick_play =
							!matches!(params.launch_config.quick_play, QuickPlayType::Realm { .. });
						if !uses_quick_play {
							return vec![];
						}
					}
				}
			}

			for arg in arg.value.iter() {
				out.extend(process_simple_arg(arg, params));
			}
		}
	};

	out
}

/// Process a simple string argument
pub(crate) fn process_simple_arg(arg: &str, params: &LaunchParameters) -> Option<String> {
	replace_arg_placeholders(arg, params)
}

/// Get the string for a placeholder token in an argument
macro_rules! placeholder {
	($name:expr) => {
		concat!("${", $name, "}")
	};
}

/// Replace placeholders in a string argument from the client meta
pub(crate) fn replace_arg_placeholders(arg: &str, params: &LaunchParameters) -> Option<String> {
	// Branding properties
	let mut out = arg.replace(
		placeholder!("launcher_name"),
		&params.branding.launcher_name,
	);
	out = out.replace(
		placeholder!("launcher_version"),
		&params.branding.launcher_version,
	);

	// Game files information
	out = out.replace(placeholder!("classpath"), &params.classpath.get_str());
	out = out.replace(
		placeholder!("natives_directory"),
		params
			.paths
			.internal
			.join("versions")
			.join(params.version.to_string())
			.join("natives")
			.to_str()?,
	);
	out = out.replace(placeholder!("version_name"), params.version);
	out = out.replace(placeholder!("version_type"), "mcvm");
	out = out.replace(placeholder!("game_directory"), params.launch_dir.to_str()?);
	out = out.replace(placeholder!("assets_root"), params.paths.assets.to_str()?);
	out = out.replace(placeholder!("assets_index_name"), params.version);
	out = out.replace(
		placeholder!("game_assets"),
		get_virtual_dir_path(params.paths).to_str()?,
	);

	out = out.replace(placeholder!("clientid"), "mcvm");
	// Apparently this is used for Twitch on older versions
	out = out.replace(placeholder!("user_properties"), "\"\"");

	// Window resolution
	let InstanceKind::Client { window } = &params.side else {
		panic!("Instance is not a client")
	};
	if let Some(WindowResolution { width, height }) = window.resolution {
		out = out.replace(placeholder!("resolution_width"), &width.to_string());
		out = out.replace(placeholder!("resolution_height"), &height.to_string());
	}

	// QuickPlayType
	out = out.replace(placeholder!("quickPlayPath"), "quickPlay/log.json");
	let quickplay_singleplayer = if let QuickPlayType::World { world } = &params.launch_config.quick_play {
		world.as_str()
	} else {
		""
	};
	let quickplay_multiplayer = if let QuickPlayType::Server { server, port } = &params.launch_config.quick_play {
		if let Some(port) = port {
			format!("{server}:{port}")
		} else {
			server.clone()
		}
	} else {
		String::new()
	};
	let quickplay_realms = if let QuickPlayType::Realm { realm } = &params.launch_config.quick_play {
		realm.as_str()
	} else {
		""
	};
	
	out = out.replace(placeholder!("quickPlaySingleplayer"), quickplay_singleplayer);
	out = out.replace(placeholder!("quickPlayMultiplayer"), &quickplay_multiplayer);
	out = out.replace(placeholder!("quickPlayRealms"), quickplay_realms);
	
	// If this argument contains quick play placeholders and all are empty, skip it entirely
	// This prevents "Only one quick play option can be specified" error in modern Minecraft (1.20+)
	// Only apply this fix to versions that actually enforce the single quick play restriction
	let needs_quickplay_fix = {
		if let (Some(current), Some(min)) = (
			version_compare::Version::from(params.version),
			version_compare::Version::from("1.20")
		) {
			let result = current >= min;
			// Debug output to understand what's happening
			eprintln!("🐛 QUICKPLAY DEBUG: version={}, needs_fix={}", params.version, result);
			result
		} else {
			// If version parsing fails, assume we need the fix for safety
			eprintln!("🐛 QUICKPLAY DEBUG: version parsing failed for {}", params.version);
			false
		}
	};
	
	if needs_quickplay_fix &&
	   (arg.contains(placeholder!("quickPlaySingleplayer")) || 
	    arg.contains(placeholder!("quickPlayMultiplayer")) || 
	    arg.contains(placeholder!("quickPlayRealms"))) &&
	   quickplay_singleplayer.is_empty() && 
	   quickplay_multiplayer.is_empty() && 
	   quickplay_realms.is_empty() {
		eprintln!("🐛 QUICKPLAY DEBUG: Skipping empty quick play argument: {}", arg);
		return None;
	}

	// User
	match params.users.get_chosen_user() {
		Some(user) => {
			// User type
			let user_type = match user.get_kind() {
				UserKind::Microsoft { .. } => "msa",
				_ => "msa",
			};
			out = out.replace(placeholder!("user_type"), user_type);

			if let Some(username) = user.get_name() {
				out = out.replace(placeholder!("auth_player_name"), username);
			}
			if let Some(uuid) = user.get_uuid() {
				out = out.replace(placeholder!("auth_uuid"), uuid);
			}
			if let Some(access_token) = user.get_access_token() {
				out = out.replace(placeholder!("auth_access_token"), &access_token.0);
			}
			if let UserKind::Microsoft {
				xbox_uid: Some(xbox_uid),
			} = &user.kind
			{
				out = out.replace(placeholder!("auth_xuid"), xbox_uid);
			}

			// Blank any args we don't replace since the game will complain if we don't
			if out.contains(placeholder!("auth_player_name"))
				|| out.contains(placeholder!("auth_access_token"))
				|| out.contains(placeholder!("auth_uuid"))
				|| out.contains(placeholder!("auth_xuid"))
			{
				return Some(String::new());
			}
		}
		None => {
			if out.contains(placeholder!("auth_player_name")) {
				return Some("UnknownUser".into());
			}
			if out.contains(placeholder!("auth_access_token"))
				|| out.contains(placeholder!("auth_uuid"))
			{
				return Some(String::new());
			}
		}
	}

	Some(out)
}

/// Create the additional game arguments for Quick Play
pub fn create_quick_play_args(
	quick_play: &QuickPlayType,
	version: &str,
	version_list: &[String],
	o: &mut impl MCVMOutput,
) -> Vec<String> {
	let mut out = Vec::new();

	match quick_play {
		QuickPlayType::World { .. }
		| QuickPlayType::Realm { .. }
		| QuickPlayType::Server { .. } => {
			let before_23w14a =
				VersionPattern::Before("23w13a".into()).matches_single(version, version_list);
			match quick_play {
				QuickPlayType::None => {}
				QuickPlayType::World { .. } => {
					if before_23w14a {
						o.display(
							MessageContents::Warning(
								"World Quick Play has no effect before 23w14a (1.20)".into(),
							),
							MessageLevel::Important,
						);
					}
				}
				QuickPlayType::Realm { .. } => {
					if before_23w14a {
						o.display(
							MessageContents::Warning(
								"Realm Quick Play has no effect before 23w14a (1.20)".into(),
							),
							MessageLevel::Important,
						);
					}
				}
				QuickPlayType::Server { server, port } => {
					if before_23w14a {
						out.push("--server".into());
						out.push(server.clone());
						if let Some(port) = port {
							out.push("--port".into());
							out.push(port.to_string());
						}
					}
				}
			}
		}
		_ => {}
	}

	out
}

/// Fill the logging path argument with the correct path
pub fn fill_logging_path_arg(arg: String, version: &str, paths: &Paths) -> Option<String> {
	let path = crate::net::game_files::log_config::get_path(version, paths);
	Some(arg.replace(placeholder!("path"), path.to_str()?))
}
