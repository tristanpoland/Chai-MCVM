#![warn(missing_docs)]

//! mcvm_pkg is a library for dealing with MCVM packages
//!
//! # Features:
//!
//! - `schema`: Enable generation of JSON schemas using the `schemars` crate

/// Standard declarative package format
pub mod declarative;
/// Package metadata
pub mod metadata;
/// Package properties
pub mod properties;
/// Standard repository format
pub mod repo;
/// Standardized package dependency resolution
pub mod resolve;
/// Framework for evaluating script packages
pub mod script_eval;

use anyhow::Context;
use async_trait::async_trait;
use declarative::{deserialize_declarative_package, validate_declarative_package};
use mcvm_shared::pkg::{ArcPkgReq, PackageID};
use metadata::PackageMetadata;
use properties::PackageProperties;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Re-export
pub use mcvm_parse as parse;
pub use mcvm_shared::pkg::{PkgRequest, PkgRequestSource};

/// Parses and validates a package
pub fn parse_and_validate(contents: &str, content_type: PackageContentType) -> anyhow::Result<()> {
	match content_type {
		PackageContentType::Script => {
			let parsed = parse::parse::lex_and_parse(contents).context("Parsing failed")?;
			metadata::eval_metadata(&parsed).context("Metadata evaluation failed")?;
			properties::eval_properties(&parsed).context("Properties evaluation failed")?;
		}
		PackageContentType::Declarative => {
			let contents = deserialize_declarative_package(contents).context("Parsing failed")?;
			validate_declarative_package(&contents).context("Package was invalid")?;
		}
	}

	Ok(())
}

/// Content type of a package
#[derive(Deserialize, Serialize, Debug, Copy, Clone, Default)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PackageContentType {
	/// A package script
	#[default]
	Script,
	/// A declarative / JSON package
	Declarative,
}

/// A required package
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequiredPackage {
	/// The package id that is required
	pub value: PackageID,
	/// Whether this is an explicit dependency
	pub explicit: bool,
}

/// A recommended package
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct RecommendedPackage {
	/// The package id that is required
	pub value: PackageID,
	/// Whether to invert this recommendation
	#[serde(default)]
	#[serde(skip_serializing_if = "is_false")]
	pub invert: bool,
}

fn is_false(x: &bool) -> bool {
	!x
}

/// Trait for a central package registry that can evaluate packages
#[async_trait]
pub trait PackageEvaluator<'a> {
	/// Type passed to most functions, used for common / cached values
	type CommonInput;
	/// Type passed to the evaluation function
	type EvalInput<'b>: Clone;
	/// Result from package relationship evaluation
	type EvalRelationsResult<'b>: PackageEvalRelationsResult;
	/// Configured package type
	type ConfiguredPackage: ConfiguredPackage<EvalInput<'a> = Self::EvalInput<'a>>;

	/// Evaluate the relationships of a package
	async fn eval_package_relations(
		&mut self,
		pkg: &ArcPkgReq,
		input: &Self::EvalInput<'a>,
		common_input: &Self::CommonInput,
	) -> anyhow::Result<Self::EvalRelationsResult<'a>>;

	/// Get the properties of a package
	async fn get_package_properties<'b>(
		&'b mut self,
		pkg: &ArcPkgReq,
		common_input: &Self::CommonInput,
	) -> anyhow::Result<&'b PackageProperties>;
}

/// Trait for a user-configured package
pub trait ConfiguredPackage: Clone {
	/// Type passed to your evaluation functions
	type EvalInput<'a>: Clone;

	/// Get the package ID
	fn get_package(&self) -> ArcPkgReq;

	/// Override the EvalInput for this package based on configuration
	fn override_configured_package_input(
		&self,
		properties: &PackageProperties,
		input: &mut Self::EvalInput<'_>,
	) -> anyhow::Result<()>;
}

/// Trait for the result from evaluating a package, used for resolution
pub trait PackageEvalRelationsResult {
	/// Get the evaluated dependencies
	fn get_deps(&self) -> Vec<Vec<RequiredPackage>>;
	/// Get the evaluated conflicts
	fn get_conflicts(&self) -> Vec<PackageID>;
	/// Get the evaluated recommendations
	fn get_recommendations(&self) -> Vec<RecommendedPackage>;
	/// Get the evaluated bundled packages
	fn get_bundled(&self) -> Vec<PackageID>;
	/// Get the evaluated compats
	fn get_compats(&self) -> Vec<(PackageID, PackageID)>;
	/// Get the evaluated extensions
	fn get_extensions(&self) -> Vec<PackageID>;
}

/// Checks if a package is open source
pub fn is_open_source(meta: &PackageMetadata, properties: &PackageProperties) -> bool {
	// The property always takes precedence
	if let Some(open_source) = &properties.open_source {
		return *open_source;
	}

	// Infer from the license
	if let Some(license) = &meta.license {
		if let "ARR" | "All Rights Reserved" = license.as_str() {
			return false;
		}
	}

	true
}
