group "default" {
    targets = ["requestsautomation"]
}

target "requestsautomation" {
    context = "."
    dockerfile = "Dockerfile"
    tags = ["raynkami/rust.auto:v0.0.31"]
}
