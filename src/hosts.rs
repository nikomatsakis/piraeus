use std::collections::BTreeSet;
use std::io::BufRead;

use ssh2_config::{ParseRule, SshConfig};

const HOST_DISCOVERY_RULES: ParseRule =
    ParseRule::ALLOW_UNKNOWN_FIELDS.union(ParseRule::ALLOW_UNSUPPORTED_FIELDS);

// {impl}`requirements.forwarding.host_discovery`
pub fn discover_hosts() -> Result<Vec<String>, String> {
    let config = SshConfig::parse_default_file(HOST_DISCOVERY_RULES)
        .map_err(|error| format!("Could not read ~/.ssh/config: {error}"))?;
    Ok(literal_hosts(&config))
}

pub fn parse_hosts(reader: &mut impl BufRead) -> Result<Vec<String>, String> {
    let config = SshConfig::default()
        .parse(reader, HOST_DISCOVERY_RULES)
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

    // {test}`requirements.forwarding.host_discovery`
    #[test]
    fn match_section_does_not_hide_later_host_declarations() {
        let text = b"Host chronos\n  HostName chronos.example\nMatch host chronos\n  IdentityFile ~/.ssh/example\nHost mnemosyne\n  HostName mnemosyne.example\n";
        let hosts = parse_hosts(&mut &text[..]).unwrap();
        assert_eq!(hosts, ["chronos", "mnemosyne"]);
    }
}
