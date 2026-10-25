// {impl}`requirements.forwarding.port_validation`
pub fn parse_port(input: &str) -> Result<u16, &'static str> {
    let text = input.trim();
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("Enter a whole-number port from 1 through 65535.");
    }
    let port = text
        .parse::<u16>()
        .map_err(|_| "Port must be from 1 through 65535.")?;
    if port == 0 {
        return Err("Port must be from 1 through 65535.");
    }
    Ok(port)
}

// {impl}`requirements.forwarding.detect_conflict`
// {impl}`requirements.forwarding.suggestion_exhausted`
pub fn first_free_after(preferred: u16, mut is_free: impl FnMut(u16) -> bool) -> Option<u16> {
    ((u32::from(preferred) + 1)..=u32::from(u16::MAX))
        .map(|port| port as u16)
        .find(|&port| is_free(port))
}

#[cfg(test)]
mod tests {
    use super::*;

    // {test}`requirements.forwarding.port_validation`
    #[test]
    fn validates_port_range() {
        assert_eq!(parse_port("8030"), Ok(8030));
        assert_eq!(parse_port(" 65535 "), Ok(65535));
        for invalid in ["", "abc", "1.5", "0", "65536", "-1"] {
            assert!(parse_port(invalid).is_err(), "{invalid}");
        }
    }

    // {test}`requirements.forwarding.detect_conflict`
    #[test]
    fn suggestion_checks_ports_in_order() {
        let mut checked = Vec::new();
        let suggestion = first_free_after(8030, |port| {
            checked.push(port);
            port == 8032
        });
        assert_eq!(suggestion, Some(8032));
        assert_eq!(checked, [8031, 8032]);
    }

    // {test}`requirements.forwarding.suggestion_exhausted`
    #[test]
    fn suggestion_stops_at_max_port() {
        assert_eq!(
            first_free_after(65535, |_| panic!("should not probe")),
            None
        );
        assert_eq!(first_free_after(65534, |_| false), None);
    }
}
