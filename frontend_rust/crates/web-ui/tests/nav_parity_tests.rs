use regex_lite::Regex;
use web_ui::{ROUTE_FAMILIES, route_family};

const NAV_CONFIG: &str = include_str!("../../../../frontend_next/lib/navigation/nav-config.ts");

fn nav_config_entries(src: &str) -> Vec<(String, String)> {
    let start = src
        .find("export const APP_NAV_ENTRIES")
        .expect("APP_NAV_ENTRIES");
    let rest = &src[start..];
    let end = rest.find("];").expect("APP_NAV_ENTRIES end");
    let body = &rest[..end];
    let id_re = Regex::new(r#"id:\s*"([^"]+)""#).unwrap();
    let href_re = Regex::new(r#"href:\s*"([^"]+)""#).unwrap();
    let mut out = Vec::new();
    for block in body.split('{').skip(1) {
        let Some(id) = id_re.captures(block).map(|cap| cap[1].to_string()) else {
            continue;
        };
        let Some(href) = href_re.captures(block).map(|cap| cap[1].to_string()) else {
            continue;
        };
        out.push((id, href));
    }
    out
}

#[test]
fn route_families_match_next_nav_config() {
    let entries = nav_config_entries(NAV_CONFIG);
    assert!(
        !entries.is_empty(),
        "failed to parse APP_NAV_ENTRIES from nav-config.ts"
    );

    let ids: Vec<_> = ROUTE_FAMILIES.iter().map(|family| family.id).collect();
    assert_eq!(ids.len(), ids.iter().collect::<std::collections::HashSet<_>>().len());

    for (id, href) in &entries {
        let family = route_family(id).unwrap_or_else(|| panic!("missing route family for nav id {id}"));
        assert_eq!(family.href, href.as_str(), "href drift for {id}");
    }

    for family in ROUTE_FAMILIES {
        assert!(
            entries.iter().any(|(id, _)| id == family.id),
            "route family {} is not in nav-config.ts",
            family.id
        );
    }
}
