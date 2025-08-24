use mcvm_auth::mc::{call_mc_api, Keypair};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Struct for a Minecraft Profile from the Minecraft Services API
#[derive(Deserialize, Serialize, Debug)]
pub struct MinecraftUserProfile {
	/// The username of this user
	pub name: String,
	/// The UUID of this user
	#[serde(rename = "id")]
	pub uuid: String,
	/// The list of skins that this user has
	pub skins: Vec<Skin>,
	/// The list of capes that this user has
	pub capes: Vec<Cape>,
}

/// A skin for a Minecraft user
#[derive(Deserialize, Serialize, Debug)]
pub struct Skin {
	/// Common cosmetic data for the skin
	#[serde(flatten)]
	pub cosmetic: Cosmetic,
	/// What variant of skin this is
	pub variant: SkinVariant,
}

/// Variant for a skin
#[derive(Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum SkinVariant {
	/// The classic wide-arm player model
	Classic,
	/// The newer slim player model
	Slim,
}

/// A cape for a Minecraft user
#[derive(Deserialize, Serialize, Debug)]
pub struct Cape {
	/// Common cosmetic data for the cape
	#[serde(flatten)]
	pub cosmetic: Cosmetic,
	/// The codename for this cape, such as 'migrator'
	pub alias: String,
}

/// Common structure used for a user cosmetic (skins and capes)
#[derive(Deserialize, Serialize, Debug)]
pub struct Cosmetic {
	/// The ID of this cosmetic
	pub id: String,
	/// The URL to the cosmetic image file
	pub url: String,
	/// The state of the cosmetic
	pub state: CosmeticState,
}

/// State for a cosmetic of whether it is active or not
#[derive(Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum CosmeticState {
	/// The cosmetic is active and being used
	Active,
	/// The cosmetic is not active
	Inactive,
}

/// Get a Minecraft user profile
pub async fn get_user_profile(
	access_token: &str,
	client: &Client,
) -> anyhow::Result<MinecraftUserProfile> {
	call_mc_api(
		"https://api.minecraftservices.com/minecraft/profile",
		access_token,
		client,
	)
	.await
}

/// Response from the player certificate endpoint
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftUserCertificate {
	/// Public / private key pair
	pub key_pair: Keypair,
}

/// Get a Minecraft user certificate
pub async fn get_user_certificate(
	access_token: &str,
	client: &Client,
) -> anyhow::Result<MinecraftUserCertificate> {
	let response = client
		.post("https://api.minecraftservices.com/player/certificates")
		.header("Authorization", format!("Bearer {access_token}"))
		.send()
		.await?
		.error_for_status()?
		.json()
		.await?;

	Ok(response)
}
