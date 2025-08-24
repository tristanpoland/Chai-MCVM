use std::collections::HashSet;
use std::path::PathBuf;

use crate::pkg::reg::CachingStrategy;
use crate::pkg::repo::{PkgRepo, PkgRepoLocation};
use mcvm_core::net::download::validate_url;

use anyhow::{bail, Context};
use mcvm_shared::lang::Language;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Configured user preferences
#[derive(Debug)]
pub struct ConfigPreferences {
	/// Caching strategy for packages
	pub package_caching_strategy: CachingStrategy,
	/// The global language
	pub language: Language,
}

/// Deserialization struct for user preferences
#[derive(Deserialize, Serialize, Default)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(default)]
pub struct PrefDeser {
	/// The user's configured repositories
	pub repositories: RepositoriesDeser,
	/// The user's configured strategy for package caching
	pub package_caching_strategy: CachingStrategy,
	/// The user's configured language
	pub language: Language,
}

/// Deserialization struct for a package repo
#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct RepoDeser {
	/// The ID of the repository
	pub id: String,
	/// The URL to the repository, which may not exist
	#[serde(skip_serializing_if = "Option::is_none")]
	pub url: Option<String>,
	/// The Path to the repository, which may not exist
	#[serde(skip_serializing_if = "Option::is_none")]
	pub path: Option<String>,
	/// Whether to disable the repo and not add it to the list
	#[serde(default)]
	pub disable: bool,
}

/// Deserialization struct for all configured package repositories
#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(default)]
pub struct RepositoriesDeser {
	/// The preferred repositories over the default ones
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub preferred: Vec<RepoDeser>,
	/// The backup repositories included after the default ones
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub backup: Vec<RepoDeser>,
	/// Whether to enable the core repository
	pub enable_core: bool,
	/// Whether to enable the std repository
	pub enable_std: bool,
}

impl Default for RepositoriesDeser {
	fn default() -> Self {
		Self {
			preferred: Vec::new(),
			backup: Vec::new(),
			enable_core: true,
			enable_std: true,
		}
	}
}

impl ConfigPreferences {
	/// Convert deserialized preferences to the stored format and returns
	/// a list of repositories to add.
	pub fn read(prefs: &PrefDeser) -> anyhow::Result<(Self, Vec<PkgRepo>)> {
		let mut repositories = Vec::new();
		for repo in prefs.repositories.preferred.iter() {
			if !repo.disable {
				add_repo(&mut repositories, repo)?;
			}
		}
		repositories.extend(PkgRepo::default_repos(
			prefs.repositories.enable_core,
			prefs.repositories.enable_std,
		));
		for repo in prefs.repositories.backup.iter() {
			if !repo.disable {
				add_repo(&mut repositories, repo)?;
			}
		}

		// Check for duplicate IDs
		let mut existing = HashSet::new();
		for repo in &repositories {
			if existing.contains(&repo.id) {
				bail!("Duplicate repository ID '{}'", repo.id);
			}
			existing.insert(&repo.id);
		}

		Ok((
			Self {
				package_caching_strategy: prefs.package_caching_strategy.clone(),
				language: prefs.language,
			},
			repositories,
		))
	}
}

/// Add a repo to the list
fn add_repo(repos: &mut Vec<PkgRepo>, repo: &RepoDeser) -> anyhow::Result<()> {
	let location = if let Some(url) = &repo.url {
		validate_url(url).with_context(|| {
			format!("Invalid url '{}' in package repository '{}'", url, repo.id)
		})?;
		PkgRepoLocation::Remote(url.clone())
	} else if let Some(path) = &repo.path {
		PkgRepoLocation::Local(PathBuf::from(path))
	} else {
		bail!("Niether path nor URL was set for repository {}", repo.id);
	};
	repos.push(PkgRepo::new(&repo.id, location));
	Ok(())
}
