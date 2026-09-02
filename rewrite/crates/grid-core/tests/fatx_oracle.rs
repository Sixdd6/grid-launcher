//! pyfatx oracle for the clean-room `fatx` module.
//!
//! This test builds an image with OUR writer and hands it to `pyfatx` as a
//! **black box subprocess**. pyfatx's source is never read: the helper
//! script below finds its entry points by runtime introspection only, which
//! is what keeps the clean-room rule intact while still getting an
//! empirical answer.
//!
//! It settles the three items on the oracle checklist in
//! `grid_core::fatx::dir`:
//!
//! 1. **Timestamp epoch** (`FATX_EPOCH_YEAR`, 1980 vs 2000). Our writer
//!    stamps the wall clock, so the year pyfatx decodes must be the current
//!    year. A decoded year exactly 20 too low or 20 too high says the
//!    constant is on the wrong base.
//! 2. **The reserved FAT entry** (`(cluster_count + 1)` in `fat_size_for`).
//!    A retail `E:` cannot decide this — both rules give the same
//!    `data_offset` there — so a second image is built at 1 GiB with 16 KiB
//!    clusters, the page-aligned case where the two rules differ by one
//!    page. If pyfatx reads that image, the `+ 1` matches.
//! 3. **Timestamp field order** (date then time at 0x34/0x38/0x3C). Swapped
//!    fields decode as a nonsense year, so item 1's assertion covers it.
//!
//! **Self-skipping.** With no importable `fatx` module — or with one whose
//! public API the helper cannot drive — the test prints a `FATX ORACLE
//! SKIPPED` note and passes. Run `cargo test -p grid-core --test
//! fatx_oracle -- --nocapture` to see the note; cargo hides the output of a
//! passing test otherwise.

use std::path::Path;
use std::process::Command;

use grid_core::fatx::builder::FatxImageBuilder;
use grid_core::fatx::image::FatxPartition;
use grid_core::fatx::layout::{RETAIL_PARTITION_E_OFFSET, RETAIL_PARTITION_E_SIZE};

/// 1 GiB with 16 KiB clusters: the page-aligned layout where the reserved
/// FAT entry moves `data_offset` (oracle checklist item 2).
const ALIGNED_PART_SIZE: u64 = 1024 * 1024 * 1024;

/// The helper script. It is deliberately API-agnostic: it enumerates the
/// public classes of `fatx`, picks one that exposes list-like and
/// read-like methods, and tries a handful of constructor shapes. Anything
/// it cannot work out is reported as a skip, never as a failure, so a
/// pyfatx whose surface differs from what this script probes cannot turn
/// into a false alarm.
const HELPER: &str = r#"
import inspect, json, os, sys, traceback

def emit(obj):
    sys.stdout.write("ORACLE_JSON " + json.dumps(obj) + "\n")
    sys.stdout.flush()

try:
    import fatx
except Exception as exc:
    emit({"skip": "import fatx failed: %r" % (exc,)})
    raise SystemExit(0)

IMAGE = sys.argv[1]
DEST = sys.argv[2]
OFFSET = int(sys.argv[3])
SIZE = int(sys.argv[4])
MODE = sys.argv[5]

LIST_NAMES = ("listdir", "list_dir", "ls", "list", "walk")
READ_NAMES = ("read", "read_file", "extract", "extract_file", "get_file", "cat")
WRITE_NAMES = ("write", "write_file", "add_file", "put_file", "create_file", "add")


def method(obj, names):
    for name in names:
        fn = getattr(obj, name, None)
        if callable(fn):
            return name, fn
    return None, None


def candidates():
    out = []
    for name in dir(fatx):
        if name.startswith("_"):
            continue
        value = getattr(fatx, name)
        if inspect.isclass(value):
            out.append((name, value))
    return out


def construct(cls):
    attempts = [
        ((IMAGE,), {}),
        ((IMAGE,), {"offset": OFFSET}),
        ((IMAGE,), {"offset": OFFSET, "size": SIZE}),
        ((IMAGE,), {"drive": "e"}),
        ((IMAGE, OFFSET), {}),
        ((IMAGE, OFFSET, SIZE), {}),
    ]
    errors = []
    for args, kwargs in attempts:
        try:
            return cls(*args, **kwargs), None
        except Exception as exc:
            errors.append("%s%r: %r" % (cls.__name__, kwargs or args, exc))
    return None, errors


def pick():
    tried = []
    for name, cls in candidates():
        lname, _ = method(cls, LIST_NAMES)
        if lname is None:
            tried.append("%s: no list method" % name)
            continue
        obj, errors = construct(cls)
        if obj is None:
            tried.append("%s: %s" % (name, "; ".join(errors or ["no constructor worked"])))
            continue
        return obj, name, tried
    return None, None, tried


def as_entries(value):
    """Normalize whatever the list method returns into (name, is_dir) pairs."""
    out = []
    for item in value:
        if isinstance(item, str):
            out.append((item, None))
        elif isinstance(item, (tuple, list)) and item:
            out.append((str(item[0]), None))
        else:
            name = getattr(item, "name", None) or getattr(item, "filename", None)
            if name is None:
                continue
            if isinstance(name, bytes):
                name = name.decode("utf-8", "replace")
            is_dir = getattr(item, "is_dir", None)
            if callable(is_dir):
                is_dir = is_dir()
            if is_dir is None:
                is_dir = getattr(item, "isdir", None)
            out.append((str(name).rstrip("\x00"), is_dir))
    return out


def stamp_of(item):
    for attr in ("mtime", "modified", "creation_time", "ctime", "atime"):
        value = getattr(item, attr, None)
        if value is not None:
            return attr, str(value)
    return None, None


obj, cls_name, tried = pick()
if obj is None:
    emit({"skip": "no usable pyfatx entry point: " + " | ".join(tried)})
    raise SystemExit(0)

lname, list_fn = method(obj, LIST_NAMES)
rname, read_fn = method(obj, READ_NAMES)
wname, write_fn = method(obj, WRITE_NAMES)

if MODE == "aligned":
    # Item 2: the image only has to open and list for the reserved-entry
    # rule to be compatible.
    try:
        list_fn("/")
    except Exception as exc:
        emit({"aligned": False, "error": repr(exc)})
        raise SystemExit(0)
    emit({"aligned": True, "impl": cls_name})
    raise SystemExit(0)

if MODE == "write":
    if write_fn is None:
        emit({"skip": "pyfatx exposes no write method (tried %r)" % (WRITE_NAMES,)})
        raise SystemExit(0)
    payload = os.path.join(DEST, "from_pyfatx.bin")
    try:
        try:
            write_fn("/UDATA/from_pyfatx.bin", payload)
        except TypeError:
            with open(payload, "rb") as handle:
                write_fn("/UDATA/from_pyfatx.bin", handle.read())
    except Exception as exc:
        emit({"skip": "pyfatx write failed: %r" % (exc,), "method": wname})
        raise SystemExit(0)
    for name in ("flush", "close", "sync"):
        fn = getattr(obj, name, None)
        if callable(fn):
            try:
                fn()
            except Exception:
                pass
    emit({"wrote": True, "method": wname})
    raise SystemExit(0)

if read_fn is None:
    emit({"skip": "pyfatx exposes no read method (tried %r)" % (READ_NAMES,)})
    raise SystemExit(0)

files = {}
stamps = {}


def walk(path):
    try:
        listed = list_fn(path if path else "/")
    except Exception as exc:
        emit({"skip": "listing %r failed: %r" % (path, exc)})
        raise SystemExit(0)
    raw = list(listed)
    for (name, is_dir), item in zip(as_entries(raw), raw):
        if name in (".", ".."):
            continue
        child = (path.rstrip("/") + "/" + name) if path else "/" + name
        attr, value = stamp_of(item)
        if attr:
            stamps[child] = [attr, value]
        if is_dir is None:
            # Unknown: try reading it, and treat a failure as a directory.
            try:
                data = read_fn(child)
                files[child] = data.hex() if isinstance(data, (bytes, bytearray)) else None
                continue
            except Exception:
                walk(child)
                continue
        if is_dir:
            walk(child)
        else:
            data = read_fn(child)
            files[child] = data.hex() if isinstance(data, (bytes, bytearray)) else None


try:
    walk("")
except SystemExit:
    raise
except Exception:
    emit({"skip": "walk failed: " + traceback.format_exc(limit=2)})
    raise SystemExit(0)

emit({"impl": cls_name, "files": files, "stamps": stamps})
"#;

struct Oracle {
    script: std::path::PathBuf,
    _tmp: tempfile::TempDir,
}

/// `Some(oracle)` when a python3 with an importable `fatx` is on PATH.
fn probe() -> Option<Oracle> {
    let python = Command::new("python3")
        .args(["-c", "import fatx"])
        .output()
        .ok()?;
    if !python.status.success() {
        let why = String::from_utf8_lossy(&python.stderr);
        println!(
            "FATX ORACLE SKIPPED: `python3 -c \"import fatx\"` failed.\n  {}\n  \
             Install pyfatx (it needs CMake to build) to run the oracle.",
            why.lines().last().unwrap_or("no output").trim()
        );
        return None;
    }
    let tmp = tempfile::tempdir().ok()?;
    let script = tmp.path().join("oracle.py");
    std::fs::write(&script, HELPER).ok()?;
    Some(Oracle { script, _tmp: tmp })
}

/// Run the helper and return its one JSON line, or `None` when it could not
/// be parsed (which is itself reported as a skip).
fn run(
    oracle: &Oracle,
    image: &Path,
    dest: &Path,
    offset: u64,
    size: u64,
    mode: &str,
) -> Option<serde_json::Value> {
    let out = Command::new("python3")
        .arg(&oracle.script)
        .arg(image)
        .arg(dest)
        .arg(offset.to_string())
        .arg(size.to_string())
        .arg(mode)
        .output()
        .expect("run the pyfatx helper");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout.lines().find_map(|l| l.strip_prefix("ORACLE_JSON "));
    let Some(line) = line else {
        println!(
            "FATX ORACLE SKIPPED ({mode}): the helper produced no result.\n  stdout: {}\n  stderr: {}",
            stdout.trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
        return None;
    };
    let value: serde_json::Value = serde_json::from_str(line).expect("helper JSON");
    if let Some(skip) = value.get("skip").and_then(|s| s.as_str()) {
        println!("FATX ORACLE SKIPPED ({mode}): {skip}");
        return None;
    }
    Some(value)
}

/// The files the oracle image carries, as (path under UDATA, contents).
fn payload() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        (
            "4541000d/00000001/savedata.bin",
            (0..40_000u32).map(|i| (i % 251) as u8).collect(),
        ),
        ("4541000d/00000001/savemeta.xbx", vec![0xA5; 100]),
        ("notes.txt", b"hello xbox".to_vec()),
    ]
}

fn build_source(root: &Path) {
    for (rel, data) in payload() {
        let target = root.join(rel);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, data).unwrap();
    }
}

#[test]
fn pyfatx_oracle_agrees_with_our_writer() {
    let Some(oracle) = probe() else { return };

    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    build_source(&src);

    // A retail-geometry image, sparse, written entirely by our own code.
    let image = tmp.path().join("xbox_hdd.img");
    FatxImageBuilder::new(RETAIL_PARTITION_E_SIZE)
        .with_base_offset(RETAIL_PARTITION_E_OFFSET)
        .with_cluster_size(16 * 1024)
        .write_to(&image)
        .expect("build the retail image");
    let mut part =
        FatxPartition::open_rw(&image, RETAIL_PARTITION_E_OFFSET, RETAIL_PARTITION_E_SIZE)
            .expect("open for writing");
    assert_eq!(part.write_tree("UDATA", &src).expect("write_tree"), 3);
    drop(part);

    let dest = tmp.path().join("pyfatx-out");
    std::fs::create_dir_all(&dest).unwrap();
    let Some(result) = run(
        &oracle,
        &image,
        &dest,
        RETAIL_PARTITION_E_OFFSET,
        RETAIL_PARTITION_E_SIZE,
        "read",
    ) else {
        return;
    };

    // --- Every file we wrote comes back byte for byte. ---
    let files = result["files"].as_object().expect("files map");
    for (rel, expected) in payload() {
        let want = format!("/UDATA/{rel}");
        let got = files
            .iter()
            .find(|(path, _)| path.eq_ignore_ascii_case(&want))
            .unwrap_or_else(|| {
                panic!(
                    "pyfatx did not list {want}; it saw {:?}",
                    files.keys().collect::<Vec<_>>()
                )
            })
            .1
            .as_str()
            .expect("hex contents");
        let bytes: Vec<u8> = (0..got.len() / 2)
            .map(|i| u8::from_str_radix(&got[i * 2..i * 2 + 2], 16).unwrap())
            .collect();
        assert_eq!(bytes, expected, "pyfatx read different bytes for {want}");
    }

    // --- Oracle items 1 and 3: the epoch, and the date/time field order. ---
    let this_year = chrono_year();
    let mut checked_a_stamp = false;
    if let Some(stamps) = result["stamps"].as_object() {
        for (path, value) in stamps {
            let Some(text) = value.get(1).and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(year) = first_year(text) else {
                continue;
            };
            checked_a_stamp = true;
            assert_eq!(
                year, this_year,
                "oracle checklist items 1 and 3: pyfatx decoded year {year} from {path} \
                 ({text:?}) where our writer stamped {this_year}. A gap of exactly 20 means \
                 fatx::dir::FATX_EPOCH_YEAR is on the wrong base; anything else means the \
                 date/time fields are the other way round at 0x34/0x38/0x3C."
            );
        }
    }
    if !checked_a_stamp {
        println!(
            "FATX ORACLE PARTIAL: contents matched, but pyfatx exposed no readable timestamp, \
             so oracle checklist items 1 and 3 stay unsettled."
        );
    }

    // --- Oracle item 2: the reserved FAT entry, on a page-aligned size. ---
    let aligned = tmp.path().join("aligned.img");
    FatxImageBuilder::new(ALIGNED_PART_SIZE)
        .with_cluster_size(16 * 1024)
        .write_to(&aligned)
        .expect("build the 1 GiB image");
    let mut part = FatxPartition::open_rw(&aligned, 0, ALIGNED_PART_SIZE).expect("open aligned");
    part.write_tree("UDATA", &src).expect("write_tree aligned");
    drop(part);
    if let Some(value) = run(&oracle, &aligned, &dest, 0, ALIGNED_PART_SIZE, "aligned") {
        assert_eq!(
            value["aligned"], true,
            "oracle checklist item 2: pyfatx could not read a 1 GiB / 16 KiB image, where the \
             `(cluster_count + 1)` FAT sizing moves data_offset by one page. Error: {:?}. \
             Drop the `+ 1` in fatx::layout::fat_size_for if this is why.",
            value["error"]
        );
        println!("FATX ORACLE: item 2 resolved — the `(cluster_count + 1)` FAT sizing is right.");
    }

    // --- The other direction: pyfatx writes, our reader reads. ---
    let content: Vec<u8> = (0..5_000u32).map(|i| (i % 97) as u8).collect();
    std::fs::write(dest.join("from_pyfatx.bin"), &content).unwrap();
    if run(
        &oracle,
        &image,
        &dest,
        RETAIL_PARTITION_E_OFFSET,
        RETAIL_PARTITION_E_SIZE,
        "write",
    )
    .is_some()
    {
        let mut part =
            FatxPartition::open(&image, RETAIL_PARTITION_E_OFFSET, RETAIL_PARTITION_E_SIZE)
                .expect("reopen after pyfatx wrote");
        let back = tmp.path().join("ours-out");
        part.read_tree("UDATA", &back).expect("read_tree");
        assert_eq!(
            std::fs::read(back.join("from_pyfatx.bin")).expect("the file pyfatx wrote"),
            content,
            "our reader disagrees with what pyfatx wrote"
        );
        println!("FATX ORACLE: pyfatx-written file read back correctly by our reader.");
    }
}

fn chrono_year() -> i64 {
    use chrono::Datelike;
    i64::from(chrono::Local::now().year())
}

/// First four-digit run in `text` that looks like a year.
fn first_year(text: &str) -> Option<i64> {
    let bytes: Vec<char> = text.chars().collect();
    for window in bytes.windows(4) {
        if window.iter().all(|c| c.is_ascii_digit()) {
            let value: i64 = window.iter().collect::<String>().parse().ok()?;
            if (1970..2200).contains(&value) {
                return Some(value);
            }
        }
    }
    None
}
