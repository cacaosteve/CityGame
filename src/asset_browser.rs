//! Read-only loopback asset browser. Media routes are an allowlist built by the catalog.
use crate::asset_catalog::Catalog;
use anyhow::{Context, Result, bail, ensure};
use std::ffi::OsString;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

const HTML: &[u8] = include_bytes!("asset_browser/index.html");
const CSS: &[u8] = include_bytes!("asset_browser/browser.css");
const JS: &[u8] = include_bytes!("asset_browser/browser.js");

pub fn run_cli(args: Vec<OsString>) -> Result<bool> {
    if args.is_empty() {
        return Ok(false);
    }
    if args == ["--help"] || args == ["-h"] {
        println!(
            "CityGame\n  (no arguments)                  Run the game\n  --asset-browser                 Serve a local asset browser; Ctrl-C to stop\n  --asset-pack PATH               Add an extracted or replacement pack (repeatable)\n  --asset-catalog FILE            Export the catalog as JSON\n  --artist-brief FILE             Export technical requirements without original art\n  --port NUMBER                  Browser port (default: select an available port)\n\nWithout --asset-pack, discover existing workspace exports and SC2000_ASSET_ROOT.\nThis tool inventories extracted packs; it does not import raw media or change game graphics."
        );
        return Ok(true);
    }
    let mut roots = vec![];
    let mut browse = false;
    let mut catalog_output = None;
    let mut brief_output = None;
    let mut port = 0;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.to_str() {
            Some("--asset-browser") => browse = true,
            Some("--asset-pack") => roots.push(PathBuf::from(
                iter.next().context("--asset-pack requires a directory")?,
            )),
            Some("--asset-catalog") => {
                catalog_output = Some(PathBuf::from(
                    iter.next().context("--asset-catalog requires a file")?,
                ))
            }
            Some("--artist-brief") => {
                brief_output = Some(PathBuf::from(
                    iter.next().context("--artist-brief requires a file")?,
                ))
            }
            Some("--port") => {
                port = iter
                    .next()
                    .context("--port requires a number")?
                    .to_string_lossy()
                    .parse::<u16>()?
            }
            _ => bail!("Unknown argument {}; use --help", arg.to_string_lossy()),
        }
    }
    ensure!(
        browse || catalog_output.is_some() || brief_output.is_some(),
        "choose --asset-browser, --asset-catalog FILE or --artist-brief FILE"
    );
    if roots.is_empty() {
        roots = default_roots();
    }
    ensure!(
        !roots.is_empty(),
        "No extracted assets found. Use --asset-pack PATH to select an existing export."
    );
    let catalog = Catalog::load(&roots)?;
    let json = serde_json::to_vec(&catalog)?;
    let brief = serde_json::to_vec_pretty(&catalog.artist_brief())?;
    // Explicit outputs cannot overwrite a file inside any selected pack.
    for (path, bytes) in [(&catalog_output, &json), (&brief_output, &brief)] {
        if let Some(path) = path {
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| std::path::Path::new("."));
            let parent = parent.canonicalize()?;
            ensure!(
                !catalog.packs.iter().any(|p| parent.starts_with(&p.root)),
                "write the report outside the source packs"
            );
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .with_context(|| {
                    format!(
                        "creating report {} (choose a new filename if it already exists)",
                        path.display()
                    )
                })?;
            file.write_all(bytes)?;
            println!("Wrote {}", path.display());
        }
    }
    println!(
        "{} assets across {} packs; {} warnings",
        catalog.assets.len(),
        catalog.packs.len(),
        catalog.warnings.len()
    );
    if browse {
        serve(catalog, json, brief, port)?;
    }
    Ok(true)
}

fn default_roots() -> Vec<PathBuf> {
    let mut roots = vec![];
    if let Some(root) = std::env::var_os("SC2000_ASSET_ROOT") {
        roots.push(PathBuf::from(root));
    }
    let outputs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../outputs");
    for name in [
        "sc2000_graphics_png",
        "sc2000_win95_cd_assets_png",
        "sc2000_gog_dosbox_assets_png",
    ] {
        let path = outputs.join(name);
        if path.is_dir() {
            roots.push(path);
        }
    }
    roots
}

fn serve(catalog: Catalog, json: Vec<u8>, brief: Vec<u8>, port: u16) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let address = listener.local_addr()?;
    println!("Asset browser: http://{address}\nRead-only, local to this computer. Ctrl-C to stop.");
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                if let Err(error) = handle(&mut stream, &catalog, &json, &brief, address.port()) {
                    log::debug!("asset browser request: {error:#}");
                }
            }
            Err(error) => log::warn!("asset browser connection: {error}"),
        }
    }
    Ok(())
}

fn handle(
    stream: &mut TcpStream,
    catalog: &Catalog,
    json: &[u8],
    brief: &[u8],
    port: u16,
) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(&mut *stream);
    let mut line = String::new();
    // Read bounded headers; no request body or state-changing endpoints are supported.
    read_bounded_line(&mut reader, &mut line)?;
    let parts: Vec<_> = line.split_whitespace().collect();
    if parts.len() != 3 || parts[0] != "GET" {
        return response(
            stream,
            "405 Method Not Allowed",
            "text/plain",
            b"GET only",
            false,
            false,
        );
    }
    let path = parts[1].to_owned();
    let mut valid_host = false;
    let mut ended = false;
    for _ in 0..40 {
        line.clear();
        read_bounded_line(&mut reader, &mut line)?;
        if line == "\r\n" || line == "\n" {
            ended = true;
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("host") {
                valid_host = value.trim() == format!("127.0.0.1:{port}")
                    || value.trim() == format!("localhost:{port}");
            }
        }
    }
    if !ended || !valid_host {
        return response(
            stream,
            "403 Forbidden",
            "text/plain",
            b"Local browser only",
            false,
            false,
        );
    }
    match path.split('?').next().unwrap_or("") {
        "/" => response(
            stream,
            "200 OK",
            "text/html; charset=utf-8",
            HTML,
            false,
            false,
        ),
        "/browser.css" => response(
            stream,
            "200 OK",
            "text/css; charset=utf-8",
            CSS,
            false,
            false,
        ),
        "/browser.js" => response(
            stream,
            "200 OK",
            "text/javascript; charset=utf-8",
            JS,
            false,
            false,
        ),
        "/api/catalog" => response(stream, "200 OK", "application/json", json, false, false),
        "/api/artist-brief" => response(stream, "200 OK", "application/json", brief, false, true),
        route if route.starts_with("/media/") => {
            let file = route
                .strip_prefix("/media/")
                .and_then(|i| i.parse::<usize>().ok())
                .and_then(|i| catalog.media.get(i));
            let Some(file) = file else {
                return response(
                    stream,
                    "404 Not Found",
                    "text/plain",
                    b"Unknown asset",
                    false,
                    false,
                );
            };
            // Reject a file that has been replaced with a symlink since inventory creation.
            if file.canonicalize().ok().as_ref() != Some(file) {
                return response(
                    stream,
                    "404 Not Found",
                    "text/plain",
                    b"Asset moved; rebuild catalog",
                    false,
                    false,
                );
            }
            let mime = match file
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase()
                .as_str()
            {
                "png" => "image/png",
                "wav" => "audio/wav",
                "mid" | "midi" => "audio/midi",
                _ => "application/octet-stream",
            };
            let bytes = fs::read(file)?;
            response(stream, "200 OK", mime, &bytes, true, false)
        }
        _ => response(
            stream,
            "404 Not Found",
            "text/plain",
            b"Not found",
            false,
            false,
        ),
    }
}

fn read_bounded_line(reader: &mut impl BufRead, line: &mut String) -> Result<()> {
    use std::io::Read;
    let count = reader.take(8193).read_line(line)?;
    ensure!(
        count > 0 && count <= 8192 && line.ends_with('\n'),
        "invalid HTTP header"
    );
    Ok(())
}

fn response(
    stream: &mut TcpStream,
    status: &str,
    mime: &str,
    body: &[u8],
    cache: bool,
    download: bool,
) -> Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self'; frame-ancestors 'none'; base-uri 'none'; object-src 'none'\r\nCache-Control: {}\r\n{}\r\n",
        body.len(),
        if cache {
            "private, no-cache"
        } else {
            "no-store"
        },
        if download {
            "Content-Disposition: attachment; filename=artist-brief.json\r\n"
        } else {
            ""
        }
    )?;
    stream.write_all(body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn request(method: &str, path: &str, local_host: bool) -> String {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let catalog = Catalog {
                schema_version: 1,
                packs: vec![],
                assets: vec![],
                warnings: vec![],
                media: vec![],
            };
            handle(
                &mut stream,
                &catalog,
                b"{\"schema_version\":1}",
                b"{\"assets\":[]}",
                address.port(),
            )
            .unwrap();
        });
        let mut client = TcpStream::connect(address).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let host = if local_host {
            address.to_string()
        } else {
            "unrelated.example".into()
        };
        write!(
            client,
            "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut result = String::new();
        client.read_to_string(&mut result).unwrap();
        server.join().unwrap();
        result
    }

    #[test]
    fn asset_browser_serves_catalog_and_downloadable_brief() {
        let catalog = request("GET", "/api/catalog", true);
        assert!(catalog.starts_with("HTTP/1.1 200"));
        assert!(catalog.ends_with("{\"schema_version\":1}"));
        assert!(
            request("GET", "/api/artist-brief", true)
                .contains("Content-Disposition: attachment; filename=artist-brief.json")
        );
    }

    #[test]
    fn asset_browser_rejects_unlisted_paths_and_nonlocal_hosts() {
        assert!(request("GET", "/media/../../etc/passwd", true).starts_with("HTTP/1.1 404"));
        assert!(request("GET", "/media/999", true).starts_with("HTTP/1.1 404"));
        assert!(request("GET", "/api/catalog", false).starts_with("HTTP/1.1 403"));
    }

    #[test]
    fn asset_browser_bounds_request_headers() {
        let data = vec![b'a'; 9000];
        assert!(read_bounded_line(&mut data.as_slice(), &mut String::new()).is_err());
    }
}
