use std::collections::BTreeSet;
use std::io::BufRead;

use ssh2_config::{ParseRule, SshConfig};

// {impl}`requirements.forwarding.host_discovery`
pub fn discover_hosts() -> Result<Vec<String>, String> {
    let config = SshConfig::parse_default_file(ParseRule::ALLOW_UNSUPPORTED_FIELDS)
        .map_err(|error| format!("Could not read ~/.ssh/config: {error}"))?;
    Ok(literal_hosts(&config))
}

pub fn parse_hosts(reader: &mut impl BufRead) -> Result<Vec<String>, String> {
    let config = SshConfig::default()
        .parse(reader, ParseRule::ALLOW_UNSUPPORTED_FIELDS)
        .map_err(|error| format!("Could not parse SSH configuration: {error}"))?;
    Ok(literal_hosts(&config))
}

fn literal_hosts(config: &SshConfig) -> Vec<String> {
    let mut names = BTreeSet::new();
    for host in config.get_hosts() {
        for clause in &host.pattern {
            let name = &clause.pattern;
            if !clause.negated
                && !name.is_empty()
                && !name.starts_with('-')
                && !name.contains(['*', '?', '[', ']'])
            {
                names.insert(name.clone());
            }
        }
    }
    names.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // {test}`requirements.forwarding.host_discovery`
    #[test]
    fn picks_literal_aliases_only() {
        let text = b"Host chronos mnemosyne\n  HostName example.com\nHost *.internal !secret\nHost chronos\nHost *\n";
        let hosts = parse_hosts(&mut &text[..]).unwrap();
        assert_eq!(hosts, ["chronos", "mnemosyne"]);
    }
}
