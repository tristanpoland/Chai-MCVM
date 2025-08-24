use anyhow::{anyhow, Context};
use mcvm_core::io::json_to_file_pretty;

use crate::io::paths::Paths;
use mcvm_shared::id::{InstanceID, ProfileID};

use super::instance::InstanceConfig;
use super::package::PackageConfigDeser;
use super::profile::ProfileConfig;
use super::user::UserConfig;
use super::{Config, ConfigDeser};

/// A modification operation that can be applied to the config
pub enum ConfigModification {
	/// Adds a new user
	AddUser(String, UserConfig),
	/// Adds a new profile
	AddProfile(ProfileID, ProfileConfig),
	/// Adds a new instance
	AddInstance(InstanceID, InstanceConfig),
	/// Adds a new package to a profile
	AddPackage(ProfileID, PackageConfigDeser),
}

/// Applies modifications to the config
pub fn apply_modifications(
	config: &mut ConfigDeser,
	modifications: Vec<ConfigModification>,
) -> anyhow::Result<()> {
	for modification in modifications {
		match modification {
			ConfigModification::AddUser(id, user) => {
				config.users.insert(id, user);
			}
			ConfigModification::AddProfile(id, profile) => {
				config.profiles.insert(id, profile);
			}
			ConfigModification::AddInstance(instance_id, instance) => {
				config.instances.insert(instance_id, instance);
			}
			ConfigModification::AddPackage(profile_id, package) => {
				let profile = config
					.profiles
					.get_mut(&profile_id)
					.ok_or(anyhow!("Unknown profile '{profile_id}'"))?;
				profile.packages.add_global_package(package);
			}
		};
	}
	Ok(())
}

/// Applies modifications to the config and writes it to the config file
pub fn apply_modifications_and_write(
	config: &mut ConfigDeser,
	modifications: Vec<ConfigModification>,
	paths: &Paths,
) -> anyhow::Result<()> {
	apply_modifications(config, modifications)?;
	let path = Config::get_path(paths);
	json_to_file_pretty(path, config).context("Failed to write modified configuration")?;

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::config::user::UserVariant;

	#[test]
	fn test_user_add_modification() {
		let mut config = ConfigDeser::default();

		let user_config = UserConfig {
			variant: UserVariant::Demo {},
		};

		let modifications = vec![ConfigModification::AddUser("bob".into(), user_config)];

		apply_modifications(&mut config, modifications).unwrap();
		assert!(config.users.contains_key("bob"));
	}
}
