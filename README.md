# tadoku_stats

tadoku_stats is a simple program which scrapes the [Tadoku contest web pages](http://readmod.com/ranking) for data and prints some summary tables.

## Usage

To just grab the information and write the tables to a file you can use
```
tadoku_stats --output stats.txt
```

The program also supports writing the raw data it has pulled from the website to a json file:
```
tadoku_stats --writejson tadoku.json
```

which you can later use to print the tables instead of pulling fresh data from the website:
```
tadoku_stats --readjson tadoku.json --output stats.txt
```

This should be handy for archiving statistics from each contest so that if we add more interesting analysis later we can run it on the old data.

## Output

The program prints:

* an "Overall rankings" table listing all contest participants in order with their page counts
* for each medium (book, game, manga, etc), a table giving the top three participants in that category with their raw pages/minutes/etc scores
* for each language, a table giving the top ten for that language

Anybody who didn't read anything in a category or language isn't listed, so if for instance only three people read in German that table will have three entries, not ten.

The output is plain text with no attempt at table formatting.

## Warning for Windows users

The output file is printed with plain newline characters, not the Windows-standand CRLF sequence. Until this bug is fixed, Windows users should make sure they read the output with WordPad or some other editor that can handle Unix-style text files. (In particular, don't use Notepad, which can't, and will display everything as one long single line.)

## License

This project is licensed under the GNU GPL, version 2, or at your option, any later version -- see the [COPYING](COPYING) file for details.
