use std::{
    collections::HashMap,
    error::Error,
    fs::{self, File},
};

use chrono::{DateTime, Utc};
use rss::{Channel, Item};
use rss::{Guid, validation::Validate};
use saphyr::LoadableYamlNode;

struct Post {
    published_at: DateTime<Utc>,
    slug: String,
    url_path: String,
    title: String,
    html_body: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    for file in fs::read_dir("../public")? {
        let Ok(file) = file else {
            continue;
        };

        if file.file_type()?.is_file() {
            fs::remove_file(file.path())?;
        } else if file.file_type()?.is_dir() {
            fs::remove_dir_all(file.path())?;
        }
    }

    for file in fs::read_dir("../dist")? {
        let Ok(file) = file else {
            continue;
        };

        if file.file_type()?.is_file() {
            fs::copy(
                file.path(),
                file.path()
                    .to_str()
                    .ok_or("must convert")?
                    .to_string()
                    .replace("/dist/", "/public/"),
            )?;
        } else if file.file_type()?.is_dir() {
            panic!("can not copy directory")
        }
    }

    let posts = parse_posts()?;

    build_posts(&posts)?;
    build_feed(&posts)?;
    build_archive(&posts)?;
    build_main()?;
    build_projects()?;
    build_contact()?;
    build_cv()?;

    return Ok(());
}

fn build_posts(posts: &Vec<Post>) -> Result<(), Box<dyn Error>> {
    let template = String::from_utf8(fs::read("../template.html")?)?;

    fs::create_dir("../public/p")?;
    for post in posts {
        fs::write(
            format!("../public/p/{}.html", post.slug),
            template
                .replace("{{title}}", &post.title)
                .replace("{{body}}", &post.html_body),
        )?;
    }

    return Ok(());
}

fn build_archive(posts: &Vec<Post>) -> Result<(), Box<dyn Error>> {
    let template = String::from_utf8(fs::read("../template.html")?)?;

    let mut list = String::new();

    list.push_str("<h1>Archive</h1>");
    list.push_str(r#"<ul class="archive">"#);

    for post in posts {
        list.push_str(&format!(
            r#"
                <li>
                    <a href="{}">
                        <span class="date">{}</span> {}
                    </a>
                </li>"#,
            post.url_path,
            post.published_at.format("%Y-%m-%d"),
            post.title,
        ));
    }

    list.push_str("</ul>");

    fs::write(
        "../public/archive.html",
        template
            .replace("{{title}}", "Archive")
            .replace("{{body}}", list.as_str()),
    )?;

    return Ok(());
}

fn build_feed(posts: &Vec<Post>) -> Result<(), Box<dyn Error>> {
    let mut channel = Channel::default();

    channel.title = "Timm Preetz".into();
    channel.link = "https://timm.preetz.xyz".into();

    // Or better to really put in now?
    channel.last_build_date = Some(posts.first().unwrap().published_at.to_rfc2822());

    for post in posts {
        let mut item = Item::default();

        item.title = Some(post.title.clone());
        let mut id = Guid::default();
        id.permalink = true;
        id.value = format!("https://timm.preetz.xyz{}", post.url_path);
        item.guid = Some(id); // TODO: URL
        item.description = Some("plain text desc".into());
        item.content = Some(post.html_body.clone());
        item.pub_date = Some(post.published_at.to_rfc2822());

        channel.items.push(item);
    }

    channel.validate().unwrap(); // https://validator.w3.org/feed/#validate_by_input

    let f = File::create("../public/rss.xml")?;
    channel.pretty_write_to(f, b' ', 2)?;

    return Ok(());
}

fn parse_posts() -> Result<Vec<Post>, Box<dyn Error>> {
    let mut posts: Vec<Post> = vec![];

    for file in fs::read_dir("../posts")? {
        let Ok(file) = file else {
            continue;
        };

        if file.file_type()?.is_dir() {
            continue;
        }

        if !file
            .file_name()
            .to_str()
            .ok_or("expected file name")?
            .ends_with(".md")
        {}

        let contents = fs::read_to_string(file.path())?;

        println!("contents: {}", contents);

        let options = markdown::ParseOptions {
            constructs: markdown::Constructs {
                // code_indented: false,
                frontmatter: true,
                ..markdown::Constructs::default()
            },
            ..markdown::ParseOptions::default()
        };

        let mut md = markdown::to_mdast(&contents, &options).unwrap();

        // let custom = markdown::Constructs {
        //     // math_flow: true,
        //     // math_text: true,
        //     // frontmatter:true,
        //     ..markdown::Constructs::mdx(),
        //     // ..markdown::Options::gfm()
        // };

        let mut title: Option<String> = None;
        let mut date: Option<chrono::DateTime<Utc>> = None;

        for child in md.children().unwrap() {
            match child {
                markdown::mdast::Node::Yaml(yaml) => {
                    println!("found Front matter {:?}", yaml);

                    let docs = saphyr::Yaml::load_from_str(&yaml.value)?;

                    let date_key =
                        saphyr::Yaml::Value(saphyr::Scalar::String(std::borrow::Cow::from("date")));
                    let title_key = saphyr::Yaml::Value(saphyr::Scalar::String(
                        std::borrow::Cow::from("title"),
                    ));

                    if let Some(yaml_date) = docs[0].as_mapping().unwrap().get(&date_key) {
                        let date_string = yaml_date.as_str().unwrap();
                        println!("found date {} ", date_string);

                        let local_date =
                            chrono::NaiveDateTime::parse_from_str(date_string, "%Y-%m-%d %H:%M")
                                .unwrap();

                        date = Some(
                            local_date
                                .and_local_timezone(chrono_tz::Europe::Berlin)
                                .unwrap()
                                .to_utc(),
                        );
                    }

                    if let Some(yaml_title) = docs[0].as_mapping().unwrap().get(&title_key) {
                        title = Some(yaml_title.as_str().unwrap().to_string());
                    }
                }
                markdown::mdast::Node::Heading(heading) => {
                    println!("found heading {:?}", heading);

                    if heading.depth == 1 {
                        if let markdown::mdast::Node::Text(text) = heading.children.first().unwrap()
                        {
                            title = Some(text.value.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        let date = match date {
            Some(date) => date,
            None => continue,
        };
        let title = match title {
            Some(title) => title,
            None => continue,
        };

        // TODO: maybe best to cut off front-matter instead of parsing it back in?

        let children = md.children_mut().unwrap();
        children.remove(0);

        let markdown_string = mdast_util_to_markdown::to_markdown(&md).unwrap();
        let html = markdown::to_html_with_options(&markdown_string, &markdown::Options::default())
            .unwrap();

        println!("html: {}", html);

        let filename = file
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let slug = filename
            .replace(
                &format!(".{}", file.path().extension().unwrap().to_str().unwrap()).to_string(),
                "",
            )
            .replace(" ", "-");

        let p = Post {
            published_at: date,
            title: title,
            slug: slug.clone(),
            url_path: format!("/p/{}", slug),
            html_body: html,
        };
        posts.push(p);
    }

    // sorting by date desc
    posts.sort_by(|a, b| {
        if a.published_at > b.published_at {
            return std::cmp::Ordering::Less;
        } else {
            return std::cmp::Ordering::Greater;
        }
    });

    return Ok(posts);
}

fn build_main() -> Result<(), Box<dyn Error>> {
    let template = String::from_utf8(fs::read("../template.html")?)?;

    let source = fs::read_to_string("../index.html")?;

    fs::write(
        "../public/index.html",
        template
            .replace("{{title}}", "Homes")
            .replace("{{body}}", &source),
    )?;

    return Ok(());
}

fn build_projects() -> Result<(), Box<dyn Error>> {
    let template = String::from_utf8(fs::read("../template.html")?)?;

    let source = fs::read_to_string("../projects.md")?;

    let body = markdown::to_html(&source);

    fs::write(
        "../public/projects.html",
        template
            .replace("{{title}}", "Projects")
            .replace("{{body}}", &body),
    )?;

    return Ok(());
}

fn build_contact() -> Result<(), Box<dyn Error>> {
    let template = String::from_utf8(fs::read("../template.html")?)?;

    let source = fs::read_to_string("../contact.md")?;

    let body = markdown::to_html(&source);

    fs::write(
        "../public/contact.html",
        template
            .replace("{{title}}", "Contact")
            .replace("{{body}}", &body),
    )?;

    return Ok(());
}

fn build_cv() -> Result<(), Box<dyn Error>> {
    let template = String::from_utf8(fs::read("../template.html")?)?;

    let source = fs::read_to_string("../cv_input.yaml")?;

    let docs = saphyr::Yaml::load_from_str(&source)?;

    let mut output = String::new();

    output.push_str(
        r#"
        <div class="printOnly page-break-after">
            <img src="/timm.jpg" alt="Timm Preetz" />

            <h1>Timm Preetz</h1>
            <pre>from https://timm.preetz.xyz</pre>
        </div>
    "#,
    );

    let start_date_key =
        saphyr::Yaml::Value(saphyr::Scalar::String(std::borrow::Cow::from("startDate")));
    let end_date_key =
        saphyr::Yaml::Value(saphyr::Scalar::String(std::borrow::Cow::from("endDate")));
    let company_key =
        saphyr::Yaml::Value(saphyr::Scalar::String(std::borrow::Cow::from("company")));
    let technologies_key = saphyr::Yaml::Value(saphyr::Scalar::String(std::borrow::Cow::from(
        "technologies",
    )));
    let summary_key =
        saphyr::Yaml::Value(saphyr::Scalar::String(std::borrow::Cow::from("summary")));
    let highlights_key =
        saphyr::Yaml::Value(saphyr::Scalar::String(std::borrow::Cow::from("highlights")));
    let position_key =
        saphyr::Yaml::Value(saphyr::Scalar::String(std::borrow::Cow::from("position")));

    for entry in docs[0].as_sequence().unwrap() {
        let entry = entry.as_mapping().unwrap();

        let company = entry.get(&company_key).map(|x| x.as_str());
        let start_date = entry.get(&start_date_key).map(|x| x.as_str());
        let end_date = entry.get(&end_date_key).map(|x| x.as_str());
        let summary = entry.get(&summary_key).map(|x| x.as_str());
        let position = entry.get(&position_key).map(|x| x.as_str());
        let technologies_list = entry
            .get(&technologies_key)
            .map(|x| x.as_sequence().map(|e| e.iter().map(|x| x.as_str())))
            .unwrap()
            .unwrap();
        let highlights_list = entry
            .get(&highlights_key)
            .map(|x| x.as_sequence().map(|e| e.iter().map(|x| x.as_str())))
            .unwrap()
            .unwrap();

        output.push_str("<section>");

        output.push_str(format!("<h2>{}</h2>\n", company.unwrap().unwrap()).as_str());

        // output.push_str("<aside>\n");

        output.push_str(
            format!(
                r#"<span class="date">{} – {}</span>"#,
                start_date.unwrap().unwrap(),
                end_date.unwrap().unwrap()
            )
            .as_str(),
        );

        output.push_str(r#"<ul class="technologies">"#);
        for t in technologies_list {
            if let Some(t) = t {
                output.push_str(format!("<li>{}</li>\n", t).as_str());
            }
        }
        output.push_str("</ul>\n");
        // output.push_str("</aside>\n");

        output.push_str(&format!(
            r#"<i class="role">{}</i>"#,
            &position.unwrap().unwrap()
        ));

        // output.push_str("<br/>");
        // output.push_str(&summary.unwrap().unwrap());

        output.push_str(r#"<ul class="highlights">"#);
        for t in highlights_list {
            if let Some(t) = t {
                output.push_str(format!("<li>{}</li>\n", t).as_str());
            }
        }
        output.push_str("</ul>\n");

        output.push_str("</section>");
    }

    output.push_str("");
    fs::write(
        "../public/cv.html",
        template
            .replace("{{title}}", "CV")
            .replace("{{body}}", &output)
            .replace(
                "</head>",
                "<meta name=\"robots\" content=\"noindex\">\n</head>",
            )
            .replace("<main>", r#"<main class="cv">"#),
    )?;

    return Ok(());
}
