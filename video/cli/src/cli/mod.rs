use ulid::Ulid;

use crate::invoker::Invoker;

pub mod access_token;
pub mod display;
pub mod events;
pub mod organization;
pub mod playback_key_pair;
pub mod playback_session;
pub mod recording;
pub mod recording_config;
pub mod room;
pub mod s3_bucket;
pub mod transcoding_config;

/// A helper tool to setup the scuffle video services
#[derive(Debug, clap::Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
	/// The mode to run the cli in
	#[clap(long, env = "MODE", default_value = "auto")]
	pub mode: Mode,

	/// The video api configuration file path
	#[clap(long, env = "SCUF_CONFIG_PATH")]
	pub config_file: Option<String>,

	/// The access key id for scuffle video api
	#[clap(long, env = "SCUF_ACCESS_KEY_ID")]
	pub access_key: Option<String>,

	/// The secret access key for scuffle video api
	#[clap(long, env = "SCUF_SECRET_ACCESS_KEY")]
	pub secret_key: Option<String>,

	/// The endpoint for scuffle video api
	#[clap(long, env = "SCUF_ENDPOINT")]
	pub endpoint: Option<String>,

	/// The organization id to use for all commands
	#[clap(long, env = "SCUF_ORGANIZATION_ID")]
	pub organization_id: Option<Ulid>,

	/// Json output
	#[clap(long)]
	pub json: bool,

	#[clap(subcommand)]
	pub command: Commands,
}

#[derive(Debug, clap::ValueEnum, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
	Auto,
	Direct,
	Grpc,
}

#[derive(Debug, clap::Args)]
pub struct SubCommand<T: clap::Subcommand> {
	#[clap(subcommand)]
	pub command: T,
}

pub trait Invokable {
	async fn invoke(&self, invoker: &mut Invoker, args: &Cli) -> anyhow::Result<()>;
}

#[derive(Debug, clap::Subcommand)]
pub enum Commands {
	/// Organization commands
	Organization(SubCommand<organization::Commands>),

	/// Access token commands
	AccessToken(SubCommand<access_token::Commands>),

	/// Events commands
	Events(SubCommand<events::Commands>),

	/// Playback key pair commands
	PlaybackKeyPair(SubCommand<playback_key_pair::Commands>),

	/// Playback session commands
	PlaybackSession(SubCommand<playback_session::Commands>),

	/// Recording commands
	Recording(SubCommand<recording::Commands>),

	/// Recording config commands
	RecordingConfig(SubCommand<recording_config::Commands>),

	/// Room commands
	Room(SubCommand<room::Commands>),

	/// S3 Bucket commands
	S3Bucket(SubCommand<s3_bucket::Commands>),

	/// Transcoding config commands
	TranscodingConfig(SubCommand<transcoding_config::Commands>),
}

impl Invokable for Commands {
	async fn invoke(&self, invoker: &mut Invoker, args: &Cli) -> anyhow::Result<()> {
		match self {
			Self::Organization(cmd) => cmd.command.invoke(invoker, args).await,
			Self::AccessToken(cmd) => cmd.command.invoke(invoker, args).await,
			Self::Events(cmd) => cmd.command.invoke(invoker, args).await,
			Self::PlaybackKeyPair(cmd) => cmd.command.invoke(invoker, args).await,
			Self::PlaybackSession(cmd) => cmd.command.invoke(invoker, args).await,
			Self::Recording(cmd) => cmd.command.invoke(invoker, args).await,
			Self::RecordingConfig(cmd) => cmd.command.invoke(invoker, args).await,
			Self::Room(cmd) => cmd.command.invoke(invoker, args).await,
			Self::S3Bucket(cmd) => cmd.command.invoke(invoker, args).await,
			Self::TranscodingConfig(cmd) => cmd.command.invoke(invoker, args).await,
		}
	}
}
