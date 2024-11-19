use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
  pub token: Option<String>,
}

impl Settings {
  pub fn new() -> Self {
    Self { token: None }
  }
}

pub fn get_full_settings_file_path() -> String {
  let home = dirs::home_dir().expect("Could not determine home directory");
  let path = home.join(".scopeclient/settings.json");
  path.to_str().unwrap().to_string()
}

pub fn load_or_init_settings_on_disk() -> Settings {
  let path = get_full_settings_file_path();

  if let Ok(file) = std::fs::read_to_string(&path) {
    if let Ok(settings) = serde_json::from_str(&file) {
      return settings;
    }
  }

  let settings = Settings::new();
  let settings_json = serde_json::to_string(&settings).unwrap();

  std::fs::create_dir_all(path.rsplitn(2, '/').last().unwrap()).unwrap();
  std::fs::write(&path, settings_json).unwrap();

  settings
}
