fn main() {
    uniffi::generate_scaffolding("src/sep_tools.udl")
        .expect("failed to generate sep-tools UniFFI scaffolding");
}
