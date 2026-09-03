// Archive builders for the E2E mocks: a stored-entry ZIP writer (used by the
// mock RomM server's content fixtures) and a minimal gzipped tar writer (used
// by the mock forge's `redream` download).
//
// Plain ESM, no npm dependencies. Both writers emit spec-valid archives that
// the Rust side reads with the real `zip` / `tar` crates
// (grid-core/src/library/extract.rs), so nothing here may take shortcuts a
// conformant reader would reject.

import zlib from "node:zlib";

// --- CRC32 (used by the stored-entry zip writer below) ---------------------

const CRC_TABLE = buildCrcTable();

function buildCrcTable() {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    table[n] = c >>> 0;
  }
  return table;
}

function crc32(buf) {
  let crc = 0xffffffff;
  for (let i = 0; i < buf.length; i++) {
    crc = CRC_TABLE[(crc ^ buf[i]) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

// --- minimal stored-entry (uncompressed) ZIP writer -------------------------

/**
 * Builds a spec-valid ZIP archive from `entries` (`{name, data: Buffer}[]`).
 * Every entry is stored (compression method 0), so no compression code is
 * needed — the archive is still a real ZIP that any conformant reader
 * (including the Rust `zip` crate) can extract.
 */
export function buildZip(entries) {
  const localChunks = [];
  const centralChunks = [];
  let offset = 0;
  const DOS_DATE_1980_01_01 = 0x0021;

  for (const { name, data } of entries) {
    const nameBuf = Buffer.from(name, "utf8");
    const crc = crc32(data);

    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0); // local file header signature
    local.writeUInt16LE(20, 4); // version needed to extract
    local.writeUInt16LE(0, 6); // general purpose bit flag
    local.writeUInt16LE(0, 8); // compression method: stored
    local.writeUInt16LE(0, 10); // last mod file time
    local.writeUInt16LE(DOS_DATE_1980_01_01, 12); // last mod file date
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(data.length, 18); // compressed size
    local.writeUInt32LE(data.length, 22); // uncompressed size
    local.writeUInt16LE(nameBuf.length, 26);
    local.writeUInt16LE(0, 28); // extra field length
    localChunks.push(local, nameBuf, data);

    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0); // central file header signature
    central.writeUInt16LE(20, 4); // version made by
    central.writeUInt16LE(20, 6); // version needed to extract
    central.writeUInt16LE(0, 8);
    central.writeUInt16LE(0, 10);
    central.writeUInt16LE(0, 12);
    central.writeUInt16LE(DOS_DATE_1980_01_01, 14);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(data.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(nameBuf.length, 28);
    central.writeUInt16LE(0, 30); // extra field length
    central.writeUInt16LE(0, 32); // file comment length
    central.writeUInt16LE(0, 34); // disk number start
    central.writeUInt16LE(0, 36); // internal file attributes
    central.writeUInt32LE(0, 38); // external file attributes
    central.writeUInt32LE(offset, 42); // relative offset of local header
    centralChunks.push(central, nameBuf);

    offset += local.length + nameBuf.length + data.length;
  }

  const centralDirStart = offset;
  const centralDir = Buffer.concat(centralChunks);

  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0); // end of central dir signature
  eocd.writeUInt16LE(0, 4); // number of this disk
  eocd.writeUInt16LE(0, 6); // disk with the start of the central directory
  eocd.writeUInt16LE(entries.length, 8); // entries on this disk
  eocd.writeUInt16LE(entries.length, 10); // total entries
  eocd.writeUInt32LE(centralDir.length, 12); // size of central directory
  eocd.writeUInt32LE(centralDirStart, 16); // offset of central directory
  eocd.writeUInt16LE(0, 20); // comment length

  return Buffer.concat([...localChunks, centralDir, eocd]);
}

// --- minimal ustar tar writer (+ gzip) ---------------------------------------

/** Every tar block — header or data — is exactly this many bytes. */
export const TAR_BLOCK_BYTES = 512;

/** Writes `value` into `field` as NUL-terminated octal, right-aligned. */
function writeOctal(header, offset, length, value) {
  const digits = value.toString(8).padStart(length - 1, "0");
  header.write(digits, offset, length - 1, "ascii");
  header.writeUInt8(0, offset + length - 1);
}

/**
 * One 512-byte ustar header block for a regular file.
 * `mode` is a number (e.g. 0o755); `name` must be ASCII and at most 100
 * bytes, which every fixture here is.
 */
function tarHeader(name, data, mode, mtime) {
  const nameBuf = Buffer.from(name, "utf8");
  if (nameBuf.length > 100) {
    throw new Error(`tar member name is too long for a ustar header: ${name}`);
  }

  const header = Buffer.alloc(TAR_BLOCK_BYTES);
  header.write(name, 0, 100, "utf8"); // name
  writeOctal(header, 100, 8, mode & 0o7777); // mode
  writeOctal(header, 108, 8, 0); // uid
  writeOctal(header, 116, 8, 0); // gid
  writeOctal(header, 124, 12, data.length); // size
  writeOctal(header, 136, 12, mtime); // mtime
  header.write("        ", 148, 8, "ascii"); // checksum: spaces while summing
  header.write("0", 156, 1, "ascii"); // typeflag: regular file
  header.write("ustar\0", 257, 6, "ascii"); // magic
  header.write("00", 263, 2, "ascii"); // version
  header.write("root", 265, 32, "ascii"); // uname
  header.write("root", 297, 32, "ascii"); // gname

  let checksum = 0;
  for (const byte of header) checksum += byte;
  // Historic format: 6 octal digits, then NUL, then a space.
  header.write(checksum.toString(8).padStart(6, "0"), 148, 6, "ascii");
  header.writeUInt8(0, 154);
  header.write(" ", 155, 1, "ascii");

  return header;
}

/** Pads `data` up to the next whole TAR_BLOCK_BYTES boundary. */
function tarPadding(data) {
  const remainder = data.length % TAR_BLOCK_BYTES;
  return remainder === 0 ? Buffer.alloc(0) : Buffer.alloc(TAR_BLOCK_BYTES - remainder);
}

/**
 * Builds an uncompressed ustar archive from `entries`
 * (`{name, data: Buffer, mode?: number}[]`, `mode` defaulting to 0755 — the
 * emulator fixtures are all executables). Ends with the two zero blocks the
 * format requires.
 */
export function buildTar(entries, { mtime = 0 } = {}) {
  const chunks = [];
  for (const { name, data, mode = 0o755 } of entries) {
    chunks.push(tarHeader(name, data, mode, mtime), data, tarPadding(data));
  }
  chunks.push(Buffer.alloc(TAR_BLOCK_BYTES * 2)); // end-of-archive marker
  return Buffer.concat(chunks);
}

/** [`buildTar`], gzipped — a real `.tar.gz` (gzip magic 1f 8b). */
export function buildTarGz(entries, options) {
  return zlib.gzipSync(buildTar(entries, options));
}

// --- minimal STFS package writer ---------------------------------------------

/**
 * Byte length of the STFS header the Rust reader consumes in one shot —
 * `STFS_HEADER_LEN` (grid-core/src/library/specials/xenia.rs:18). A package
 * shorter than this is rejected before its magic is even looked at.
 */
export const STFS_HEADER_LEN = 0x368;

/** Big-endian `u32` offset of the content type field (`xenia.rs:25`). */
const STFS_CONTENT_TYPE_OFFSET = 0x344;
/** Big-endian `u32` offset of the title id field (`xenia.rs:28`). */
const STFS_TITLE_ID_OFFSET = 0x360;

/**
 * Builds a [`STFS_HEADER_LEN`]-byte fake STFS package: zero-filled, with
 * `magic` (a 4-byte ASCII string — `"CON "`, `"LIVE"` or `"PIRS"` are the
 * three the reader accepts) at offset 0, `contentType` at
 * `STFS_CONTENT_TYPE_OFFSET` and `titleId` at `STFS_TITLE_ID_OFFSET`, both
 * big-endian.
 *
 * The JS twin of `grid_core::library::specials::xenia::build_stfs_bytes`.
 * The layout is duplicated rather than shared because the mock server is a
 * plain-Node process with no way to call into the Rust crate; the two must
 * stay byte-identical, which `server.test.mjs` pins from this side and
 * `xenia.rs`'s own tests pin from the other.
 */
export function buildStfs(magic, titleId, contentType) {
  const magicBuf = Buffer.from(magic, "ascii");
  if (magicBuf.length !== 4) {
    throw new Error(`STFS magic must be exactly 4 bytes, got ${magicBuf.length}`);
  }
  const bytes = Buffer.alloc(STFS_HEADER_LEN);
  magicBuf.copy(bytes, 0);
  bytes.writeUInt32BE(contentType >>> 0, STFS_CONTENT_TYPE_OFFSET);
  bytes.writeUInt32BE(titleId >>> 0, STFS_TITLE_ID_OFFSET);
  return bytes;
}
