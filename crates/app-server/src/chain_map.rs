//! A stable, accessible node-link view of the server-selected shortest path.
use crate::{chain_view::SelectedChain, ui};

pub(crate) fn render(chain: &SelectedChain) -> String {
    format!(
        "<figure class=\"chain-map\" aria-labelledby=\"chain-map-title\"><div class=\"map-heading\"><h3 id=\"chain-map-title\">The connection, at a glance</h3><span>{} degrees · {} players</span></div><p class=\"map-help\">Select a player to see their profile. Select a team on a line to inspect the teammate evidence.</p>{}{}<figcaption>Each line is one evidenced teammate connection. Positions show the order of this shortest chain.</figcaption></figure>",
        chain.degree,
        chain.path.len(),
        diagram(chain, false),
        diagram(chain, true)
    )
}

fn diagram(chain: &SelectedChain, vertical: bool) -> String {
    let count = chain.path.len();
    let width = if vertical { 340 } else { count.max(1) * 240 };
    let height = if vertical { count.max(1) * 200 } else { 240 };
    let position = |i: usize| {
        if vertical {
            (56, 65 + i * 200)
        } else {
            (120 + i * 240, 95)
        }
    };
    let mut content = String::new();
    for (i, link) in chain.links.iter().enumerate() {
        let (x1, y1) = position(i);
        let (x2, y2) = position(i + 1);
        let (x, y) = (if vertical { 190 } else { (x1 + x2) / 2 }, (y1 + y2) / 2);
        let proof = link
            .overlap_days
            .map(|d| format!("{d} overlap days"))
            .unwrap_or_else(|| {
                link.minimum_shared_games
                    .map(|n| format!("≥ {n} shared games"))
                    .unwrap_or_else(|| "Shared team game".into())
            });
        // Vertical labels follow the player name, leaving a clear hit target.
        let badge_y = y;
        let branch = if vertical {
            format!(
                "<line x1=\"{x1}\" y1=\"{y}\" x2=\"{}\" y2=\"{y}\"/>",
                x - 58
            )
        } else {
            String::new()
        };
        let label = format!(
            "{} and {}: {}. {}. Open evidence",
            chain.path[i].name,
            chain.path[i + 1].name,
            link.team,
            proof
        );
        content.push_str(&format!(
            "<g class=\"map-connection\" data-from=\"{}\" data-to=\"{}\"><line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\"/>{branch}<a class=\"map-evidence\" href=\"/edge?from={}&amp;to={}\" aria-label=\"{}\"><title>{}</title><rect x=\"{}\" y=\"{}\" width=\"116\" height=\"44\" rx=\"22\"/><text class=\"map-team\" x=\"{x}\" y=\"{}\">{}</text><text class=\"map-proof\" x=\"{x}\" y=\"{}\">{}</text></a></g>",
            ui::escape(&link.from), ui::escape(&link.to), ui::url_encode(&link.from), ui::url_encode(&link.to), ui::escape(&label), ui::escape(&label), x - 58, badge_y - 22, badge_y + 5, ui::escape(&link.team), badge_y + 40, ui::escape(&proof)
        ));
    }
    for (i, player) in chain.path.iter().enumerate() {
        let (x, y) = position(i);
        let role = if i == 0 && count == 1 {
            "Same player"
        } else if i == 0 {
            "Starting player"
        } else if i + 1 == count {
            "Destination"
        } else {
            "Mutual teammate"
        };
        let class = if i == 0 {
            "start"
        } else if i + 1 == count {
            "destination"
        } else {
            "intermediate"
        };
        let name_x = if vertical { x + 54 } else { x };
        let name_y = if vertical { y + 2 } else { y + 62 };
        let role_y = if vertical { y + 25 } else { y + 82 };
        let align = if vertical { "start" } else { "middle" };
        let initials: String = player
            .name
            .split_whitespace()
            .filter_map(|s| s.chars().next())
            .take(3)
            .collect();
        content.push_str(&format!(
            "<a class=\"map-player {class}\" data-player-id=\"{}\" href=\"/players/{}\" aria-label=\"View {} profile\"><title>{}: {role}</title><circle cx=\"{x}\" cy=\"{y}\" r=\"34\"/><text class=\"map-initials\" x=\"{x}\" y=\"{}\">{}</text><text class=\"map-name\" style=\"text-anchor:{align}\" x=\"{name_x}\" y=\"{}\">{}</text><text class=\"map-role\" style=\"text-anchor:{align}\" x=\"{name_x}\" y=\"{}\">{role}</text></a>",
            ui::escape(&player.id), ui::url_encode(&player.id), ui::escape(&player.name), ui::escape(&player.name), y + 7, ui::escape(&initials), name_y, ui::escape(&player.name), role_y
        ));
    }
    let sizing = if vertical {
        String::new()
    } else {
        format!(" style=\"min-width:{width}px\"")
    };
    let orientation = if vertical { "mobile" } else { "desktop" };
    format!(
        "<div class=\"chain-map-{orientation}\"><svg{sizing} viewBox=\"0 0 {width} {height}\" width=\"{width}\" height=\"{height}\" role=\"group\" aria-label=\"Shortest teammate chain: {} connections\">{content}</svg></div>",
        chain.degree
    )
}
