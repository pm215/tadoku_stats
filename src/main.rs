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
extern crate reqwest;
extern crate isolang;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate clap;

#[macro_use]
extern crate maplit;

#[macro_use]
extern crate lazy_static;

use select::document::Document;
use select::predicate::{Predicate, Class, Name};
use regex::Regex;
use reqwest::Client;
use isolang::Language;

use std::collections::HashMap;
use std::fs::File;
use std::error::Error;
use std::io::Write;
use std::io::BufWriter;

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

    // Find the list of reading languages. This gets us a space-separated string
    // with them all. (We only want it if there's just a single language.)
    let langs = document.find(Class("info"))
        .map(|tag| tag.text())
        .filter(|t| t.starts_with("Reading language(s)"))
        .next().unwrap()
        .split_whitespace().skip(2).collect::<Vec<_>>().join(" ");

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

    let entry_0_copy = dataarrays[0].clone();

    let mut seriesmap: HashMap<String, Vec<f64>> =
        seriesnames.iter().cloned().zip(dataarrays).collect();

    if seriesnames.len() == 1 {
        // Single-language user, add an entry for "lang" as well as the
        // "Overall" one.
        seriesmap.insert(langs, entry_0_copy);
    }

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

fn doc_from_url(client: &Client, url: &str) -> Result<Document, Box<Error>> {
    eprintln!{"Fetching page {}...", url};
    let d = Document::from_read(client.get(url).send()?)?;
    Ok(d)
}

fn read_from_webpage() -> Result<Vec<UserInfo>, Box<Error>> {
    let mut users = Vec::new();
    let client = Client::new();

    let mainpage = doc_from_url(&client, "http://readmod.com/ranking")?;
    eprintln!{"Parsing frontpage..."};
    let userids = parse_mainpage(mainpage);
    for uid in userids {
        let userpage = doc_from_url(&client, &("http://readmod.com/users/".to_string() + &uid))?;
        eprintln!{"Parsing user page..."};
        users.push(parse_userpage(userpage));
    }
    Ok(users)
}

type ResultTable<'a> = Vec<(&'a String, f64)>;

fn get_table<F>(users: &Vec<UserInfo>, maxentries: usize, keyfn: F) -> ResultTable
    where F: Fn(&UserInfo) -> f64
{ 
    // Sort this vector of integers according to the keyfn, taking account of
    // the difficulties with sorting f64s.
    let mut usridx = (0..users.len()).collect::<Vec<_>>();
    let getv = |x: &usize| keyfn(&users[*x]);
    usridx.sort_unstable_by(|a, b| getv(b).partial_cmp(&(getv(a))).unwrap());
    if maxentries != 0 {
        usridx.truncate(maxentries);
    }
    usridx.retain(|x| getv(x) >= 0.01);

    let mut tablevec = Vec::new();

    for u in usridx {
        tablevec.push((&users[u].name, getv(&u)));
    }
    tablevec
}

fn print_table<W: Write>(ds: &mut BufWriter<W>, title: &str, table: &ResultTable, html: bool) {
    if html {
        write!(ds, "<p><h5>{}</h5>\n<p>\n", title).unwrap();
    } else {
        write!(ds, "{}\n", title).unwrap();
    }
    let brtag = if html { "<br />" } else { "" };

    for (i, &(name, value)) in table.iter().enumerate() {
        write!(ds, "{}. {} {:.2}{}\n", i + 1, name, value, brtag).unwrap();
    }
    if html {
        write!(ds, "</p>\n").unwrap();
    } else {
        write!(ds, "\n").unwrap();
    }
}

fn langcode_to_name(code: &str) -> String {
    // Return the full name of the language given its code.
    // Annoyingly we have to special case Japanese because
    // the Tadoku bot uses "jp" when the correct ISO 639-1
    // code is "ja".
    match code {
        "jp" => "Japanese",
        _ => Language::from_639_1(code).map_or("unidentified", |x| x.to_name())
    }.to_string()
}

fn lang_table_title(l: &str, table: &ResultTable) -> String {
    // Pick a title for the language table
    match table.len() {
        1 => format!("Top {} ({}) reader", langcode_to_name(l), l),
        _ => format!("Top {} {} ({}) readers", table.len(), langcode_to_name(l), l),
    }
}

// We have a preferred order for the language ranking tables:
// jp en fr es de zh ru ko vi it pt el eo nl sv hr
// with anything else in alpha order on the end
fn lang_sort_idx(s: &str) -> Option<usize> {
    let langlist = ["jp", "en", "fr", "es", "de", "zh", "ru","ko", "vi", "it", "pt", "el", "eo", "nl", "sv", "hr"];
    langlist.iter().position(|&x| x == s)
}

fn lang_comparator(a: &&String, b: &&String) -> std::cmp::Ordering {
    match (lang_sort_idx(a), lang_sort_idx(b)) {
        (Some(aidx), Some(bidx)) => aidx.cmp(&bidx),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.cmp(&b),
    }
}

lazy_static! {
    static ref MEDIUM_DESCRIPTION: HashMap<&'static str, &'static str> = hashmap!{
        "Book" => "book pages read",
        "Full Game" => "full game screens read",
        "Game" => "game screens read",
        "Lyrics" => "lyrics read",
        "Manga" => "manga pages read",
        "Net" => "net pages read",
        "News" => "news articles read",
        "Nico" => "nico watched",
        "Sentences" => "sentences read",
        "Subs" => "minutes of subs watched",
    };
    static ref MEDIUM_ACTOR: HashMap<&'static str, &'static str> = hashmap!{
        "Book" => "book reader",
        "Full Game" => "full-game reader",
        "Game" => "game reader",
        "Lyrics" => "lyric reader",
        "Manga" => "manga reader",
        "Net" => "net reader",
        "News" => "news reader",
        "Nico" => "nico reader/watcher",
        "Sentences" => "sentence reader",
        "Subs" => "subs reader/watcher",
    };
    static ref MEDIUM_UNITS: HashMap<&'static str, &'static str> = hashmap!{
        "Book" => "pages",
        "Full Game" => "screens",
        "Game" => "screens",
        "Lyrics" => "lyrics",
        "Manga" => "pages",
        "Net" => "pages",
        "News" => "articles",
        "Nico" => "nico",
        "Sentences" => "sentences",
        "Subs" => "minutes",
    };
}

// The default cases could be prettier if we incorporated the medium name,
// but in practice they'll never be used so it's not worth the effort.
fn medium_description(m: &str) -> &'static str {
    MEDIUM_DESCRIPTION.get(m).unwrap_or(&"raw counts")
}

fn medium_actor(m: &str) -> &'static str {
    MEDIUM_ACTOR.get(m).unwrap_or(&"thing reader")
}

fn medium_units(m: &str) -> &'static str {
    MEDIUM_UNITS.get(m).unwrap_or(&"raw units")
}

fn print_brief_medium_table<W: Write>(ds: &mut BufWriter<W>, m: &str, table: &ResultTable, html: bool) {
    // Just print the top two contenders for the medium, in a
    // conversational format.
    // For HTML we print the second one as a list nested inside the first,
    // which typically makes it render as indented.
    let ulli = if html { "<ul><li>" } else { "" };
    let closeulli = if html { "</ul></li>" } else { "</ul></li>" };
    match table.get(1) {
        Some(&(name, value)) =>
            write!(ds, "{}{} is our top {} with {} {}.\n", ulli,
                   name, medium_actor(m), value, medium_description(m)).unwrap(),
        None => return,
    };
    match table.get(2) {
        Some(&(name, value)) =>
            write!(ds, "{}Honorable mention goes to {} with {} {} recorded.\n{}{}\n",
                   ulli, name, value, medium_units(m), closeulli, closeulli).unwrap(),
        None => write!(ds, "{}\n", closeulli).unwrap(),
    };
}

fn print_stats(dest: Box<Write>, users: &Vec<UserInfo>, brief: bool, html: bool) {
    let mut ds = BufWriter::new(dest);

    let mut media = users.iter()
        .flat_map(|u| u.countmap.keys())
        .collect::<Vec<_>>();
    media.sort();
    media.dedup();

    let mut languages = users.iter()
        .flat_map(|u| u.seriesmap.keys())
        .filter(|x| *x != "Overall")
        .collect::<Vec<_>>();
    languages.sort_by(lang_comparator);
    languages.dedup();

    {
        let table = get_table(&users, 0, |u| u.totalpoints);
        print_table(&mut ds, "Overall rankings", &table, html);
    }

    if html {
        write!(ds, "<h4>MEDIUM CHAMPS</h4>\n\n").unwrap();
    }

    for m in media {
        let table = get_table(&users, 3, |u| *u.countmap.get(m).unwrap_or(&0.0));
        if table.len() == 0 {
            continue;
        }

        if brief {
            print_brief_medium_table(&mut ds, m, &table, html);
        } else {
            let title = format!("{} rankings ({})", m, medium_description(m));
            print_table(&mut ds, &title, &table, html);
        }
    }

    for l in languages {
        let emptyvec : Vec<f64> = Vec::new();
        let table = get_table(&users, 10,
                              |u| u.seriesmap.get(l).unwrap_or(&emptyvec).iter()
                              .fold(0.0, |sum, x| sum + x));
        if table.len() == 0 {
            continue;
        }
        let title = lang_table_title(l, &table);
        print_table(&mut ds, &title, &table, html);
    }
}

fn main() {
    let matches = clap_app!(tadoku_stats =>
                            (version: crate_version!())
                            (author: crate_authors!())
                            (about: "Print summary statistics for Tadoku contest")
                            (@arg readjson: --readjson [JSONFILE] "Read data from json file rather than the website")
                            (@arg results: --results [FILE] "Write summary statistics to file")
                            (@arg writejson: --writejson [JSONFILE] conflicts_with[readjson results] "Don't print statistics, just write raw data to a json file (for later use with --readjson)")
                            (@arg brief: --brief "Print only brief (top/honorable mention) summaries for each medium rather than full tables")
                            (@arg html: --html "Print the output as a fragment of HTML")
    ).get_matches();

    let users = if matches.is_present("readjson") {
        let jsonfile = File::open(matches.value_of("readjson").unwrap()).unwrap();
        read_json(&jsonfile)
    } else {
        read_from_webpage()
    }.unwrap();

    if matches.is_present("writejson") {
        let jsonfile = File::create(matches.value_of("writejson").unwrap()).unwrap();
        write_json(&jsonfile, &users).unwrap();
        return;
    }

    let outfile = if matches.is_present("results") {
        let filename = matches.value_of("results").unwrap();
        Box::new(File::create(filename).unwrap()) as Box<Write>
    } else {
        Box::new(std::io::stdout()) as Box<Write>
    };

    print_stats(outfile, &users, matches.is_present("brief"), matches.is_present("html"));
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
