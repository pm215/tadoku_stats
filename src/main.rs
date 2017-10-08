// tadoku_stats:
// scrape the tadoku results pages from
//   http://readmod.com/ranking
// and print statistics.

// Copyright Peter Maydell <pmaydell@chiark.greenend.org.uk>
// License: GPLv2-or-later.

extern crate select;
use select::document::Document;
use select::predicate::{Predicate, Class, Name};

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

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    extern crate select;
    use select::document::Document;
    // TODO can we just import everything from the root here?
    use parse_mainpage;

    #[test]
    fn test_parse_mainpage() {
        let document = Document::from(include_str!("ranking.html"));
        let users = parse_mainpage(document);
        // Check that we parsed our sample document plausibly
        assert_eq!(users.len(), 28);
        assert_eq!(users[0], "801");
    }
}
