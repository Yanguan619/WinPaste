fn main() {
    // For a self-contained app that carries its own runtime:
    windows_reactor_setup::as_framework_dependent();
    // Other options: as_framework_dependent(), as_example().
}
