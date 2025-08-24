use std::collections::HashMap;

use anyhow::Context;
use mcvm_core::auth_crate::mc::ClientId;
use mcvm_core::io::java::args::MemoryNum;
use mcvm_core::io::java::install::JavaInstallationKind;
use mcvm_core::user::UserManager;
use mcvm_plugin::hooks::{
	HookHandle, InstanceLaunchArg, OnInstanceLaunch, OnInstanceStop, WhileInstanceLaunch,
};
use mcvm_shared::output::{MCVMOutput, MessageContents, MessageLevel};
use mcvm_shared::translate;
use reqwest::Client;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::update::manager::UpdateManager;
use crate::config::instance::QuickPlay;
use crate::plugin::PluginManager;
use crate::io::paths::Paths;

use super::Instance;

impl Instance {
	/// Launch the instance process
	pub async fn launch(
		&mut self,
		paths: &Paths,
		users: &mut UserManager,
		plugins: &PluginManager,
		settings: LaunchSettings,
		o: &mut impl MCVMOutput,
	) -> anyhow::Result<InstanceHandle> {
		o.display(
			MessageContents::StartProcess(translate!(o, StartUpdatingInstance, "inst" = &self.id)),
			MessageLevel::Important,
		);

		let mut manager = UpdateManager::new(false, true);
		let client = Client::new();
		manager.set_version(&self.config.version);
		manager.add_requirements(self.get_requirements());
		manager.set_client_id(settings.ms_client_id);
		if settings.offline_auth {
			manager.offline_auth();
		}
		manager
			.fulfill_requirements(users, plugins, paths, &client, o)
			.await
			.context("Update failed")?;

		let result = self
			.create(&mut manager, plugins, paths, users, &client, o)
			.await
			.context("Failed to update instance")?;
		manager.add_result(result);

		let hook_arg = InstanceLaunchArg {
			id: self.id.to_string(),
			side: Some(self.get_side()),
			dir: self.dirs.get().inst_dir.to_string_lossy().into(),
			game_dir: self.dirs.get().game_dir.to_string_lossy().into(),
			version_info: manager.version_info.get_clone(),
			custom_config: self.config.plugin_config.clone(),
			pid: None,
		};

		let mut installed_version = manager
			.get_core_version(o)
			.await
			.context("Failed to get core version")?;

		let mut instance = self
			.create_core_instance(&mut installed_version, paths, o)
			.await
			.context("Failed to create core instance")?;

		// Make sure that any fluff from the update gets ended
		o.end_process();

		o.display(
			MessageContents::StartProcess(translate!(o, PreparingLaunch)),
			MessageLevel::Important,
		);

		// Run pre-launch hooks
		let results = plugins
			.call_hook(OnInstanceLaunch, &hook_arg, paths, o)
			.context("Failed to call on launch hook")?;
		for result in results {
			result.result(o)?;
		}

		// Launch the instance using core
		let handle = instance
			.launch_with_handle(o)
			.await
			.context("Failed to launch core instance")?;

		// Run while_instance_launch hooks alongside
		let hook_handles = plugins
			.call_hook(WhileInstanceLaunch, &hook_arg, paths, o)
			.context("Failed to call while launch hook")?;
		let handle = InstanceHandle {
			inner: handle,
			hook_handles,
			hook_arg,
		};

		Ok(handle)
	}
}

/// Settings for launch provided to the instance launch function
pub struct LaunchSettings {
	/// The Microsoft client ID to use
	pub ms_client_id: ClientId,
	/// Whether to do offline auth
	pub offline_auth: bool,
}

/// Options for launching after conversion from the deserialized version
#[derive(Debug)]
pub struct LaunchOptions {
	/// Java kind
	pub java: JavaInstallationKind,
	/// JVM arguments
	pub jvm_args: Vec<String>,
	/// Game arguments
	pub game_args: Vec<String>,
	/// Minimum JVM memory
	pub min_mem: Option<MemoryNum>,
	/// Maximum JVM memory
	pub max_mem: Option<MemoryNum>,
	/// Environment variables
	pub env: HashMap<String, String>,
	/// Wrapper command
	pub wrapper: Option<WrapperCommand>,
	/// Quick Play options
	pub quick_play: QuickPlay,
	/// Whether or not to use the Log4J configuration
	pub use_log4j_config: bool,
}

/// A wrapper command
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct WrapperCommand {
	/// The command to run
	pub cmd: String,
	/// The command's arguments
	#[serde(default)]
	pub args: Vec<String>,
}

/// A handle for an instance
pub struct InstanceHandle {
	/// Core InstanceHandle with the process
	inner: mcvm_core::InstanceHandle,
	/// Handles for hooks running while the instance is running
	hook_handles: Vec<HookHandle<WhileInstanceLaunch>>,
	/// Arg to pass to the stop hook when the instance is stopped
	hook_arg: InstanceLaunchArg,
}

impl InstanceHandle {
	/// Waits for the process to complete
	pub fn wait(
		mut self,
		plugins: &PluginManager,
		paths: &Paths,
		o: &mut impl MCVMOutput,
	) -> anyhow::Result<std::process::ExitStatus> {
		let result = self.inner.wait()?;
		// Kill any sibling processes now that the main one is complete
		for handle in self.hook_handles {
			handle
				.kill(o)
				.context("Failed to kill plugin sibling process")?;
		}

		Self::call_stop_hooks(&self.hook_arg, plugins, paths, o)?;

		Ok(result)
	}

	/// Kills the process early
	pub fn kill(
		mut self,
		plugins: &PluginManager,
		paths: &Paths,
		o: &mut impl MCVMOutput,
	) -> anyhow::Result<()> {
		for handle in self.hook_handles {
			handle
				.kill(o)
				.context("Failed to kill plugin sibling process")?;
		}
		self.inner
			.kill()
			.context("Failed to kill inner instance handle")?;

		Self::call_stop_hooks(&self.hook_arg, plugins, paths, o)?;

		Ok(())
	}

	/// Gets the internal child process for the game, consuming the
	/// InstanceHandle
	pub fn get_process(self) -> std::process::Child {
		self.inner.get_process()
	}

	/// Calls on stop hooks
	fn call_stop_hooks(
		arg: &InstanceLaunchArg,
		plugins: &PluginManager,
		paths: &Paths,
		o: &mut impl MCVMOutput,
	) -> anyhow::Result<()> {
		let results = plugins
			.call_hook(OnInstanceStop, arg, paths, o)
			.context("Failed to call on stop hook")?;
		for result in results {
			result.result(o)?;
		}
		Ok(())
	}
}
