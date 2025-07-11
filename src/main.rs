use chrono::Utc;
use clap::{Parser, arg};
use error::AppError;
use ini::Ini;
use serde_json::json;

mod error;
mod sts;

#[derive(Clone, Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Save credentials to this profile. Print to stdout using shell export syntax if not provided
    #[arg(short, long)]
    profile: Option<String>,

    /// Role arn to assume
    #[arg(short = 'a', long)]
    role_arn: String,

    /// Unique role session name. Will automatically add a random string as the suffix
    #[arg(short, long)]
    session_name: String,

    /// Unique role session name. Will automatically add a random string as the suffix
    #[arg(short = 'f', long, default_value = ".aws/credentials")]
    credentials_file: String,

    /// Duration of the assumed role in seconds, minimum value is 900
    #[arg(short, long, default_value_t = 3600)]
    duration: i32,

    /// Auto refresh the sts token
    #[arg(short = 'r', long, default_value_t = false)]
    refresh: bool,

    /// as credential_process
    #[arg(short = 'c', long, default_value_t = false)]
    process: bool,
}

async fn assume_role(args: Args) -> Result<(), AppError> {
    let current_time = Utc::now().format("%Y-%m-%d%H%M%SZ");
    let session_name = format!("{}@{}", args.session_name, current_time);

    let home_folder = dirs::home_dir().expect("Failed to get home folder");
    let credentials = sts::assume_role(&args.role_arn, args.duration, &session_name).await?;

    if args.process {
        let value = json!({
              "Version": 1,
              "AccessKeyId": &credentials.access_key_id,
              "SecretAccessKey": &credentials.secret_access_key,
              "SessionToken": &credentials.session_token,
              "Expiration": &credentials.expiration.to_string(),
        });
        println!("{}", value);
        return Ok(());
    }

    match args.profile {
        Some(profile) => {
            let credentials_file = home_folder.join(&args.credentials_file);
            if !credentials_file.exists() {
                std::fs::create_dir_all(credentials_file.parent().unwrap())?;
                std::fs::File::create(&credentials_file)?;
            }
            let mut credentials_config = Ini::load_from_file(&credentials_file)?;
            let mut profile_credentials = credentials_config.with_section(Some(&profile));

            profile_credentials
                .set("aws_access_key_id", &credentials.access_key_id)
                .set("aws_secret_access_key", &credentials.secret_access_key)
                .set("aws_session_token", &credentials.session_token);

            credentials_config.write_to_file(&credentials_file)?;
            println!("aws profile [{}] credentials updated", &profile);
        }

        None => {
            println!("export AWS_ACCESS_KEY_ID={}", &credentials.access_key_id);
            println!("export AWS_SECRET_ACCESS_KEY={}", &credentials.secret_access_key);
            println!("export AWS_SESSION_TOKEN={}", &credentials.session_token);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let args = Args::parse();

    if args.refresh {
        let seconds_before_refresh = 300;
        let refresh_interval = args.duration - seconds_before_refresh;
        println!("Auto refresh is enabled, refresh interval {} seconds", refresh_interval);

        loop {
            assume_role(args.clone()).await?;
            tokio::time::sleep(tokio::time::Duration::from_secs(refresh_interval as u64)).await;
        }
    } else {
        assume_role(args).await?;
    }

    Ok(())
}
