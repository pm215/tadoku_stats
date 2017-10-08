// tadoku_stats:
// scrape the tadoku results pages from
//   http://readmod.com/ranking
// and print statistics.

// Copyright Peter Maydell <pmaydell@chiark.greenend.org.uk>
// License: GPLv2-or-later.

extern crate select;
use select::document::Document;
use select::predicate::{Predicate, Class, Name};

use std::collections::HashMap;

fn parse_mainpage(document: Document) -> Vec<String> {
    // Parse the top level rankings page, the relevant part of which looks like
    //	<table class="table">
    //   <thead> ... </thead>
    //   <tbody>
    //   <tr>
    //    <td><li></td>
    // 	  <td><img .../></td>
    // 	  <td><a href="/users/801">username</a></td>
    // 	  <td>638.9</td></li>
    // 	 </tr>
    //   [etc for all entries]
    // We want to extract the username and the ID value from the link to the per-user
    // page. We'll get the pagecount for stats from the per-user page later;
    // here we just use it to filter out users who have no pages recorded.
    // We just return a list of the IDs (we will get the username and score
    // info that we use from the individual user pages).

    // For now our error handling is just to panic if we don't see what we expect.

    let mut users = Vec::new();

    let tablebody = document.find(Class("ranking").descendant(Name("tbody"))).next().unwrap();
    for trnode in tablebody.find(Name("tr")) {
        let link = trnode.find(Name("a")).next().unwrap();
        let userurl = link.attr("href").unwrap();
        let pagecount = trnode.find(Name("td")).nth(3).unwrap().text();
        let userid = userurl.split("/").last().unwrap();

        // Note that this is a string comparison...
        if pagecount != "0.0" {
            //println!{"username {} userid {} pagecount {}", username, userid, pagecount};
            users.push(String::from(userid));
        }
    }
    return users;
}

#[derive(Debug)]
struct UserInfo {
    name: String,
    countmap: HashMap<String, f64>,
    totalpoints: f64,
}

fn parse_userpage(document: Document) -> UserInfo {
    // We want to grab:
    //  * the raw page counts for each category from the content tab
    //  * the total point value
    //  * the daily series data from the javascript
    // TODO: maybe we should get the main page for each language instead?
    let username = document.find(Class("avatar")).next().unwrap().attr("alt").unwrap();

    let tablehead = document.find(Class("table-bordered").descendant(Name("thead"))).next().unwrap();
    let tablebody = document.find(Class("table-bordered").descendant(Name("tbody"))).next().unwrap();
    // Pull the category names out of the table head. We discard the first <th> (empty)
    // and the last ("Total")
    let headings = tablehead.find(Name("th"))
        .map(|tag| tag.text())
        .skip(1)
        .filter(|x| x != "Total")
        .collect::<Vec<_>>();
    // First <tr> in here has the raw-page counts
    let rawcounts = tablebody.find(Name("tr")).next().unwrap()
        .find(Name("td"))
        .map(|tag| tag.text())
        .skip(1)
        .filter(|x| x != "")
        .map(|x| x.parse::<f64>().unwrap())
        .collect::<Vec<_>>();
    // Create a category -> count hashtable
    let countmap: HashMap<String, f64> =
        headings.iter().cloned().zip(rawcounts).collect();

    // Now get the total point value out of the 2nd <tr>
    let totalpoints = tablebody.find(Name("tr")).nth(1).unwrap()
        .find(Name("td"))
        .last().unwrap()
        .text()
        .parse().unwrap();

    UserInfo {
        name : String::from(username),
        countmap: countmap,
        totalpoints : totalpoints,
    }
}

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    extern crate select;
    use select::document::Document;
    // TODO can we just import everything from the root here?
    use parse_mainpage;
    use parse_userpage;

    #[test]
    fn test_parse_mainpage() {
        let document = Document::from(include_str!("ranking.html"));
        let users = parse_mainpage(document);
        // Check that we parsed our sample document plausibly
        assert_eq!(users.len(), 28);
        assert_eq!(users[0], "801");
    }

    #[test]
    fn test_parse_userpage() {
        let document = Document::from(include_str!("userpage.html"));
        let user = parse_userpage(document);
        println!{"{:#?}", user};
        assert_eq!(user.name, "shenmedemo");
        assert_eq!(user.totalpoints, 638.9);
        assert_eq!(user.countmap.len(), 10);
        let bookcount = user.countmap.get("Book").unwrap();
        assert!(bookcount == &91.0);
    }
}
