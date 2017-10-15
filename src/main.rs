// tadoku_stats:
// scrape the tadoku results pages from
//   http://readmod.com/ranking
// and print statistics.

// Copyright Peter Maydell <pmaydell@chiark.greenend.org.uk>
// License: GPLv2-or-later.

extern crate select;
extern crate regex;
extern crate serde;
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

use select::document::Document;
use select::predicate::{Predicate, Class, Name};
use regex::Regex;

use std::collections::HashMap;
use std::fs::File;
use std::error::Error;

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

#[derive(Debug, Serialize, Deserialize)]
struct UserInfo {
    name: String,
    countmap: HashMap<String, f64>,
    seriesmap: HashMap<String, Vec<f64>>,
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

    let js = document.find(Name("script"))
        .map(|tag| tag.text())
        .filter(|t| t.contains("progress_chart"))
        .next().unwrap();

    // Within the JS nodes we have to fish stuff out by regex.
    // Firstly, if the text doesn't include "progress_chart"
    // it's the wrong script node.

    // We're looking for a bit of js like this:
    //   series: [{
    //    name: "Overall",
    //    pointInterval: 86400000,
    //    pointStart: 1506816000000,
    //    data: [294.20000000000005, 0, 8.0, 57.6, 77.6, 88.00000000000001, 68.0, 45.5, 0]
    //   }, {
    //    name: "jp",
    //    pointInterval: 86400000,
    //    pointStart: 1506816000000,
    //    data: [285.20000000000005, 0, 0, 51.6, 77.6, 83.00000000000001, 41.0, 21.0, 0]
    //   }]
    // which has one entry for Overall and one for each language. We assume the
    // info is always per-day and just go for the data arrays.
    let seriesnames = Regex::new(r#"name: "([^"]*)""#).unwrap()
        .captures_iter(&*js)
        .map(|caps| caps[1].to_string())
        .collect::<Vec<_>>();

    let dataarrays = Regex::new(r#"data: \[([^\]]*)\]"#).unwrap()
        .captures_iter(&*js)
        .map(|caps| caps[1].to_string())
        .map(|ds| ds.split(",")
             .map(|s| s.trim().parse::<f64>().unwrap())
             .collect::<Vec<_>>())
        .collect::<Vec<_>>();

    let seriesmap: HashMap<String, Vec<f64>> =
        seriesnames.iter().cloned().zip(dataarrays).collect();

    UserInfo {
        name : String::from(username),
        countmap: countmap,
        seriesmap: seriesmap,
        totalpoints : totalpoints,
    }
}

// NB that serializing and deserializing can make tiny rounding errors
// on the floating point data; for instance an f64 294.20000000000005 will end up
// in the JSON as 294.20000000000007 and then read back as 294.2000000000001.
// We don't care because we're going to round them all off anyway, and
// besides the low bits are the result of rounding errors in the server
// side software that produced the data we're parsing in the first place.

fn write_json(file: &File, users: &Vec<UserInfo>) -> Result<(), Box<Error>> {
    serde_json::to_writer(file, users)?;
    Ok(())
}

fn read_json(file: &File) -> Result<Vec<UserInfo>, Box<Error>> {
    let users = serde_json::from_reader(file)?;
    Ok(users)
}

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    extern crate select;
    extern crate tempfile;
    use select::document::Document;
    use std::fs::File;
    use std::io::{Seek, SeekFrom};
    // TODO can we just import everything from the root here?
    use parse_mainpage;
    use parse_userpage;
    use write_json;
    use read_json;

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
        assert_eq!(user.seriesmap.len(), 4);
        assert_eq!(user.seriesmap.get("jp").unwrap().len(), 9);
    }

    #[test]
    fn test_write_read_json() {
        let document = Document::from(include_str!("userpage.html"));
        let mut users = Vec::new();
        users.push(parse_userpage(document));
        let mut tmpfile: File = tempfile::tempfile().unwrap();
        write_json(&tmpfile, &users).unwrap();
        tmpfile.seek(SeekFrom::Start(0)).unwrap();
        // To see the raw json for the test:
        // Add use::stdio::Read to our dependencies, and:
        //  let mut buf = String::new();
        //  tmpfile.read_to_string(&mut buf).unwrap();
        //  println!{"raw json: {}", buf};
        let readusers = read_json(&tmpfile).unwrap();
        assert_eq!(readusers.len(), users.len());
        assert_eq!(readusers[0].name, users[0].name);
    }
}
