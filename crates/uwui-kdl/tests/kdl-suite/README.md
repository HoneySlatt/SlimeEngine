# KDL's official tests

The tests of KDL's specification, from [kdl-org/kdl](https://github.com/kdl-org/kdl), its folder
`tests/test_cases`, at the commit `f49027875a673b9a0b2c8587de3ea01959ab66bb` of 2026-08-10. Its
specification at that commit, `draft-marchan-kdl2.md`, is the one that `uwui-kdl` follows; where
the tests and the specification differ, the specification wins.

- `input/`: the documents. A name ending in `_fail` is a document that a reader must refuse.
- `expected_kdl/`: each valid document in the normal form that `SUITE-README.md` describes.
- `SUITE-README.md`: the suite's own description, `tests/README.md` at the same commit.
- `LICENSE.md`: their licence, Creative Commons Attribution-ShareAlike 4.0 International, which is
  not uwui's: these files stay out of uwui's package (`exclude` in `../../Cargo.toml`). Their
  authors are KDL's contributors.

The files are kept byte for byte (`.gitattributes`): some test a carriage return or a byte order
mark. To take a later commit, copy the same folder from it, write its hash here, and run
`cargo test -p uwui-kdl --test suite`.
