use crate::PayloadError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOptions {
    pub host: Option<String>,
    pub username: Option<String>,
    pub port: u16,
}

impl CliOptions {
    pub fn parse<I, S>(args: I) -> Result<Self, PayloadError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let args: Vec<String> = args.into_iter().map(Into::into).collect();
        let mut options = Self {
            host: None,
            username: None,
            port: 22,
        };
        let mut index = 0;
        while index < args.len() {
            let target = match args[index].as_str() {
                "--host" => &mut options.host,
                "--user" => &mut options.username,
                "--port" => {
                    index += 1;
                    let port = args
                        .get(index)
                        .ok_or(PayloadError::Invalid)?
                        .parse::<u16>()
                        .map_err(|_| PayloadError::Invalid)?;
                    if port == 0 {
                        return Err(PayloadError::Invalid);
                    }
                    options.port = port;
                    index += 1;
                    continue;
                }
                _ => return Err(PayloadError::Invalid),
            };
            index += 1;
            let value = args.get(index).ok_or(PayloadError::Invalid)?;
            if value.is_empty() {
                return Err(PayloadError::Invalid);
            }
            *target = Some(value.clone());
            index += 1;
        }
        Ok(options)
    }
}
