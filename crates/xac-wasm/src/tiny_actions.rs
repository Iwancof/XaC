use anyhow::{anyhow, Result};

#[derive(Clone, Debug)]
pub(crate) enum Arg {
    Ident(String),
    Number(String),
    String(String),
}

pub(crate) fn action_to_script(line: usize, name: &str, args: &[Arg]) -> Result<String> {
    match name {
        "noop" => {
            require_arg_count(line, name, args, 0)?;
            Ok("noop".to_string())
        }
        "mine" => {
            require_arg_count(line, name, args, 0)?;
            Ok("mine".to_string())
        }
        "output" => {
            require_arg_count(line, name, args, 1)?;
            Ok(format!("output {}", ident_arg(line, name, args, 0)?))
        }
        "push_any" => {
            require_arg_count(line, name, args, 0)?;
            Ok("push_any".to_string())
        }
        "push" => match args {
            [] => Ok("push_any".to_string()),
            [Arg::Ident(dir)] => Ok(format!("push {}", normalize_ident(dir))),
            [Arg::Ident(item), Arg::Ident(dir)] => Ok(format!(
                "push {} {}",
                normalize_ident(item),
                normalize_ident(dir)
            )),
            _ => Err(anyhow!(
                "line {line}: push expects (), (dir), or (item, dir)"
            )),
        },
        "set_recipe" => {
            require_arg_count(line, name, args, 1)?;
            Ok(format!("set_recipe {}", ident_arg(line, name, args, 0)?))
        }
        "produce" => {
            require_arg_count(line, name, args, 0)?;
            Ok("produce".to_string())
        }
        "attack_nearest" => {
            require_arg_count(line, name, args, 0)?;
            Ok("attack_nearest".to_string())
        }
        "attack" => {
            require_arg_count(line, name, args, 1)?;
            Ok(format!("attack {}", number_arg(line, name, args, 0)?))
        }
        "attack_best" => {
            if args.is_empty() {
                return Err(anyhow!(
                    "line {line}: attack_best expects at least one policy"
                ));
            }
            let policies = args
                .iter()
                .map(|arg| match arg {
                    Arg::Ident(value) => Ok(normalize_ident(value)),
                    _ => Err(anyhow!(
                        "line {line}: attack_best policies must be identifiers"
                    )),
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(format!("attack_best {}", policies.join(" ")))
        }
        "dispatch" => {
            require_arg_count(line, name, args, 0)?;
            Ok("dispatch".to_string())
        }
        "charge_docked_drones" => {
            require_arg_count(line, name, args, 0)?;
            Ok("charge_docked_drones".to_string())
        }
        "create_delivery_job" => {
            require_arg_count(line, name, args, 3)?;
            Ok(format!(
                "create_delivery_job {} {} {}",
                ident_arg(line, name, args, 0)?,
                number_arg(line, name, args, 1)?,
                ident_arg(line, name, args, 2)?
            ))
        }
        "dispatch_idle_drones" => {
            require_arg_count(line, name, args, 0)?;
            Ok("dispatch_idle_drones".to_string())
        }
        "return_to_port" => {
            require_arg_count(line, name, args, 0)?;
            Ok("return_to_port".to_string())
        }
        "claim_delivery_job" => {
            require_arg_count(line, name, args, 0)?;
            Ok("claim_delivery_job".to_string())
        }
        "deliver" => {
            require_arg_count(line, name, args, 0)?;
            Ok("deliver".to_string())
        }
        "move_to" => {
            require_arg_count(line, name, args, 2)?;
            Ok(format!(
                "move_to {} {}",
                number_arg(line, name, args, 0)?,
                number_arg(line, name, args, 1)?
            ))
        }
        "load" => {
            require_arg_count(line, name, args, 2)?;
            Ok(format!(
                "load {} {}",
                ident_arg(line, name, args, 0)?,
                number_arg(line, name, args, 1)?
            ))
        }
        "unload" => {
            require_arg_count(line, name, args, 2)?;
            Ok(format!(
                "unload {} {}",
                ident_arg(line, name, args, 0)?,
                number_arg(line, name, args, 1)?
            ))
        }
        "idle" => {
            require_arg_count(line, name, args, 0)?;
            Ok("idle".to_string())
        }
        "net_set" => {
            require_arg_count(line, name, args, 2)?;
            Ok(format!(
                "net_set {} {}",
                number_arg(line, name, args, 0)?,
                number_arg(line, name, args, 1)?
            ))
        }
        "net_delete" | "net_del" => {
            require_arg_count(line, name, args, 1)?;
            Ok(format!("net_delete {}", number_arg(line, name, args, 0)?))
        }
        "log" => {
            require_arg_count(line, name, args, 1)?;
            let message = match &args[0] {
                Arg::String(value) => value.trim(),
                Arg::Ident(value) => value.trim(),
                Arg::Number(value) => value.trim(),
            };
            if message.is_empty() || message.contains('#') || message.contains("//") {
                return Err(anyhow!(
                    "line {line}: log message must be non-empty and cannot contain comments"
                ));
            }
            Ok(format!("log {message}"))
        }
        _ => Err(anyhow!("line {line}: unknown action function {name}")),
    }
}

pub(crate) fn require_arg_count(
    line: usize,
    name: &str,
    args: &[Arg],
    expected: usize,
) -> Result<()> {
    if args.len() == expected {
        Ok(())
    } else {
        Err(anyhow!(
            "line {line}: {name} expects {expected} arguments, got {}",
            args.len()
        ))
    }
}

pub(crate) fn ident_arg(line: usize, name: &str, args: &[Arg], index: usize) -> Result<String> {
    match args.get(index) {
        Some(Arg::Ident(value)) => Ok(normalize_ident(value)),
        _ => Err(anyhow!(
            "line {line}: argument {} to {name} must be an identifier",
            index + 1
        )),
    }
}

pub(crate) fn number_arg(line: usize, name: &str, args: &[Arg], index: usize) -> Result<String> {
    match args.get(index) {
        Some(Arg::Number(value)) => Ok(value.clone()),
        _ => Err(anyhow!(
            "line {line}: argument {} to {name} must be a number",
            index + 1
        )),
    }
}

pub(crate) fn normalize_ident(value: &str) -> String {
    value.replace('-', "_").to_ascii_lowercase()
}
