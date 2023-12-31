#![allow(dead_code)]

// ASCII
// -- CONTROL CHARACTERS
pub const NULL: u8 = 0x00; // NULL
pub const SOH: u8 = 0x01; // START OF HEADER
pub const STX: u8 = 0x02; // START OF TEXT
pub const ETX: u8 = 0x03; // END OF TEXT
pub const EOT: u8 = 0x04; //
pub const ANQ: u8 = 0x05;
pub const ACK: u8 = 0x06;
pub const BEL: u8 = 0x07;
pub const BS: u8 = 0x08; // BACKSPACE
pub const HT: u8 = 0x09; // horizontal tab
pub const LF: u8 = 0x0A;
pub const VT: u8 = 0x0B;
pub const FF: u8 = 0x0C;
pub const CR: u8 = 0x0D;
pub const SO: u8 = 0x0E;
pub const SI: u8 = 0x0F;
pub const DLE: u8 = 0x10;
pub const DC1: u8 = 0x11;
pub const DC2: u8 = 0x12;
pub const DC3: u8 = 0x13;
pub const DC4: u8 = 0x14;
pub const NAK: u8 = 0x15;
pub const SYN: u8 = 0x16;
pub const ETB: u8 = 0x17;
pub const CAN: u8 = 0x18;
pub const EM: u8 = 0x19;
pub const SUB: u8 = 0x1A;
pub const ESC: u8 = 0x1B;
pub const FS: u8 = 0x1C;
pub const GS: u8 = 0x1D;
pub const RS: u8 = 0x1E;
pub const US: u8 = 0x1F;
pub const DEL: u8 = 0x7F;

// --  GRAPHIC CHARACTERS
pub const SP: u8 = 0x20; // space
pub const EXCLAMATION: u8 = 0x21; // !
pub const DQUOTE: u8 = 0x22; // "
pub const NUM: u8 = 0x23; // #
pub const DOLLAR: u8 = 0x24; // $
pub const PERCENT: u8 = 0x25; // %
pub const AMPERSAND: u8 = 0x26; // &
pub const SQUOTE: u8 = 0x27; // '
pub const LEFT_PAR: u8 = 0x28; // (
pub const RIGHT_PAR: u8 = 0x29; // )
pub const ASTERISK: u8 = 0x2A; // *
pub const PLUS: u8 = 0x2B; // +
pub const COMMA: u8 = 0x2C; // ,
pub const MINUS: u8 = 0x2D; // -
pub const PERIOD: u8 = 0x2E; // .
pub const SLASH: u8 = 0x2F; // /
pub const N0: u8 = 0x30; // 0
pub const N1: u8 = 0x31; // 1
pub const N2: u8 = 0x32; // 2
pub const N3: u8 = 0x33; // 3
pub const N4: u8 = 0x34; // 4
pub const N5: u8 = 0x35; // 5
pub const N6: u8 = 0x36; // 6
pub const N7: u8 = 0x37; // 7
pub const N8: u8 = 0x38; // 8
pub const N9: u8 = 0x39; // 9
pub const COL: u8 = 0x3A; // :
pub const SEM_COL: u8 = 0x3B; // ;
pub const LT: u8 = 0x3C; // <
pub const EQ: u8 = 0x3D; // =
pub const GT: u8 = 0x3E; // >
pub const QUESTION: u8 = 0x3F; // ?
pub const AT: u8 = 0x40; // @
pub const LCA: u8 = 0x41; // A
pub const LCB: u8 = 0x42; // B
pub const LCC: u8 = 0x43; // C
pub const LCD: u8 = 0x44; // D
pub const LCE: u8 = 0x45; // E
pub const LCF: u8 = 0x46; // F
pub const LCG: u8 = 0x47; // G
pub const LCH: u8 = 0x48; // H
pub const LCI: u8 = 0x49; // I
pub const LCJ: u8 = 0x4A; // J
pub const LCK: u8 = 0x4B; // K
pub const LCL: u8 = 0x4C; // L
pub const LCM: u8 = 0x4D; // M
pub const LCN: u8 = 0x4E; // N
pub const LCO: u8 = 0x4F; // O
pub const LCP: u8 = 0x50; // P
pub const LCQ: u8 = 0x51; // Q
pub const LCR: u8 = 0x52; // R
pub const LCS: u8 = 0x53; // S
pub const LCT: u8 = 0x54; // T
pub const LCU: u8 = 0x55; // U
pub const LCV: u8 = 0x56; // V
pub const LCW: u8 = 0x57; // W
pub const LCX: u8 = 0x58; // X
pub const LCY: u8 = 0x59; // Y
pub const LCZ: u8 = 0x5A; // Z
pub const LEFT_BRACKET: u8 = 0x5B; // [
pub const BACKSLASH: u8 = 0x5C; // \
pub const RIGHT_BRACKET: u8 = 0x5D; // ]
pub const CARRET: u8 = 0x5E; // ^
pub const UNDERSCORE: u8 = 0x5F; // _
pub const GRAVE: u8 = 0x60; // `
pub const LSA: u8 = 0x61; // a
pub const LSB: u8 = 0x62; // b
pub const LSC: u8 = 0x63; // c
pub const LSD: u8 = 0x64; // d
pub const LSE: u8 = 0x65; // e
pub const LSF: u8 = 0x66; // f
pub const LSG: u8 = 0x67; // g
pub const LSH: u8 = 0x68; // h
pub const LSI: u8 = 0x69; // i
pub const LSJ: u8 = 0x6A; // j
pub const LSK: u8 = 0x6B; // k
pub const LSL: u8 = 0x6C; // l
pub const LSM: u8 = 0x6D; // m
pub const LSN: u8 = 0x6E; // n
pub const LSO: u8 = 0x6F; // o
pub const LSP: u8 = 0x70; // p
pub const LSQ: u8 = 0x71; // q
pub const LSR: u8 = 0x72; // r
pub const LSS: u8 = 0x73; // s
pub const LST: u8 = 0x74; // t
pub const LSU: u8 = 0x75; // u
pub const LSV: u8 = 0x76; // v
pub const LSW: u8 = 0x77; // w
pub const LSX: u8 = 0x78; // x
pub const LSY: u8 = 0x79; // y
pub const LSZ: u8 = 0x7A; // z
pub const LEFT_CURLY: u8 = 0x7B; // {
pub const PIPE: u8 = 0x7C; // |
pub const RIGHT_CURLY: u8 = 0x7D; // }
pub const TILDE: u8 = 0x7E; // ~

// GROUP OF CHARACTERS
// -- CRLF
pub const CRLF: &[u8] = &[CR, LF];
// -- CRCRLF
// Sometimes portable libraries replace transparently
// the "\n" with "\r\n" on Windows. When developpers
// explicitly write "\r\n", the library generates "\r\r\n".
pub const CRCRLF: &[u8] = &[CR, CR, LF];

// -- WHITESPACE
pub const WS: &[u8] = &[HT, SP];

pub const GRAPHIC_BEGIN: u8 = SP;
pub const GRAPHIC_END: u8 = TILDE;
