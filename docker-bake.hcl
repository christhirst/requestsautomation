group "default" {
    targets = ["requestsautomation"]
}

target "requestsautomation" {
    context = "."
    dockerfile = "Dockerfile"
    tags = ["rust.auto:latest"]
}
