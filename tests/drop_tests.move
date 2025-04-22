#[test_only]
module large::drop_tests;

use large::drop as drop;

#[test]
fun test_drop() {
    let sender = @0xaed0c050e900c014e115ee0bcf89128268190ef9d74dd315644dcefd535037cf;

    let root = x"1f04aec6d96959b8de0a28eac5ef3fd959de11210f4ec398bc3955d9a22d0d13";
    let proof_bts =
        x"0c20b26d90ee82f51bb06729a234348bf292318c995acf87a7ff8ce427f4d8fe40bf20be5606af14b7115528e4dd37ec4198fbbb66611452068b86ddf6aa7bf21385bb20841594dd674084e6701710bb47feaa197b4d1d4cf9e056b70d56c0d5396a46a820f57190b5bc3ab73ca9b9516b36a607a24e7ee8ef571df6f53ecfe234dccc713120e43f881478d94873af8420f167374749aa0ee1e3cecc6853a706c2043fccb18720bc4d3066eae4ec371f609d7464fc1809ef9d114c4986b4eb9005d8800260d70220384cb972f063ba5fb9ee1e378acae2b7977fb733b342acc7252490430c2094032077b0bced3f1024754da840f32308b7ad5299eb31d7ee0283a20232a43700419720a3c37a0f5bfb72934d07fbb36057a5c26b77fa9e4594ddc2e83c919bea547ede20b87133e4a1359e9aaf23b86e3bcb734ee7e1b430780beca30f243b3a729a4db920e92f4b3db03797e5caa08c506ed62a3bc82b9a0ab522b2ab316f420b946c420c207ec4cea64f7092bac715c09da7dd1d4f48ad59a026556cbf895be7a65b50af5f";

    let mut deserializer = sui::bcs::new(proof_bts);
    let proof = sui::bcs::peel_vec_vec_u8(&mut deserializer);
    let leaf_index = 1692;
    let allocation = 76000000000;

    let res = drop::verify_sender_proof(
        root,
        &proof,
        &sender,
        allocation,
        leaf_index,
    );
    assert!(res);
}
