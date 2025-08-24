use crate::net::download;
use mcvm_shared::util::{ARCH_STRING, OS_STRING, PREFERRED_ARCHIVE};

use anyhow::{anyhow, Context};
use reqwest::Client;

/// Downloading Adoptium JDK
pub mod adoptium {
	use anyhow::bail;
	use serde::Deserialize;

	use super::*;

	/// Gets the newest Adoptium binaries download for a major Java version
	pub async fn get_latest(major_version: &str, client: &Client) -> anyhow::Result<PackageFormat> {
		let url = json_url(major_version);
		let mut manifest = download::json::<Vec<PackageFormat>>(&url, client)
			.await
			.context("Failed to download manifest of Adoptium versions")?;
		if manifest.is_empty() {
			bail!("A valid installation was not found");
		}
		let version = manifest.swap_remove(0);

		Ok(version)
	}

	/// Gets the URL to the JSON file for a major Java version
	fn json_url(major_version: &str) -> String {
		format!(
			"https://api.adoptium.net/v3/assets/latest/{major_version}/hotspot?image_type=jre&vendor=eclipse&architecture={}&os={}",
			get_arch_arg(),
			get_os_arg(),
		)
	}

	/// Get the OS argument for the API
	fn get_os_arg() -> &'static str {
		if cfg!(target_os = "macos") {
			"mac"
		} else {
			OS_STRING
		}
	}

	/// Get the arch argument for the API
	fn get_arch_arg() -> &'static str {
		if cfg!(target_arch = "x86_64") {
			"x64"
		} else {
			ARCH_STRING
		}
	}

	/// A single package info for Adoptium
	#[derive(Deserialize, Debug)]
	pub struct PackageFormat {
		/// Information about the binary
		pub binary: Binary,
		/// Name of the Java release
		pub release_name: String,
	}

	/// Binary for an Adoptium package
	#[derive(Deserialize, Debug)]
	pub struct Binary {
		/// Package field that contains the download link
		pub package: BinaryPackage,
	}

	/// Package field inside the binary struct
	#[derive(Deserialize, Debug)]
	pub struct BinaryPackage {
		/// Link to the JRE download
		pub link: String,
	}
}

/// Downloading Azul Zulu
pub mod zulu {
	use super::*;

	use mcvm_shared::util::preferred_archive_extension;
	use serde::Deserialize;

	/// Gets the newest Zulu package for a major Java version
	pub async fn get_latest(major_version: &str, client: &Client) -> anyhow::Result<PackageFormat> {
		let url = json_url(major_version);
		let manifest = download::json::<Vec<PackageFormat>>(&url, client)
			.await
			.context("Failed to download manifest of Zulu versions")?;
		let package = manifest
			.first()
			.ok_or(anyhow!("A valid installation was not found"))?;

		Ok(package.to_owned())
	}

	/// Gets the URL to the JSON file for a major Java version
	fn json_url(major_version: &str) -> String {
		format!(
			"https://api.azul.com/metadata/v1/zulu/packages/?java_version={major_version}&os={OS_STRING}&arch={ARCH_STRING}&archive_type={PREFERRED_ARCHIVE}&java_package_type=jre&latest=true&java_package_features=headfull&release_status=ga&availability_types=CA&certifications=tck&page=1&page_size=100"
		)
	}

	/// Format of the metadata JSON with download info for Zulu
	#[derive(Deserialize, Clone)]
	pub struct PackageFormat {
		/// Name of the Zulu version
		pub name: String,
		/// Download URL for the package
		pub download_url: String,
	}

	/// Gets the name of the extracted directory by removing the archive file extension
	pub fn extract_dir_name(name: &str) -> String {
		name.replacen(&preferred_archive_extension(), "", 1)
	}

	#[cfg(test)]
	mod tests {
		use super::*;

		#[test]
		fn test_extract_dir_name() {
			let name = format!("hello.{PREFERRED_ARCHIVE}");
			assert_eq!(extract_dir_name(&name), "hello");
		}
	}
}

/// Downloading GraalVM
pub mod graalvm {
	use bytes::Bytes;
	use mcvm_shared::util::preferred_archive_extension;

	use super::*;

	/// Downloads the contents of the GraalVM archive
	pub async fn get_latest(major_version: &str, client: &Client) -> anyhow::Result<Bytes> {
		let url = download_url(major_version);
		download::bytes(url, client).await
	}

	/// Gets the download URL
	fn download_url(major_version: &str) -> String {
		format!(
			"https://download.oracle.com/graalvm/{major_version}/latest/graalvm-jdk-{major_version}_{}-{}_bin{}",
			OS_STRING,
			ARCH_STRING,
			preferred_archive_extension()
		)
	}
}
