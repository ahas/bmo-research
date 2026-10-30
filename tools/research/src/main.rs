//! The research tool's executable: one request handed by the tool host on standard input, answered on
//! standard output (`docs/api.md`, 「The tools」).

use std::io::Read as _;
use std::process::ExitCode;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
  let mut raw = String::new();
  if let Err(why) = std::io::stdin().read_to_string(&mut raw) {
    eprintln!("the request did not read: {why}");
    return ExitCode::FAILURE;
  }
  let handed = match serde_json::from_str(&raw) {
    Ok(v) => v,
    Err(why) => {
      eprintln!("the request is not JSON: {why}");
      return ExitCode::FAILURE;
    }
  };
  println!("{}", research_tool::answer(handed).await);
  ExitCode::SUCCESS
}
