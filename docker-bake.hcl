variable "TAG" {
    default = "v0.0.34"
}

group "default" {
    targets = ["requestsautomation"]
}

target "requestsautomation" {
    context = "."
    dockerfile = "Dockerfile"
    tags = [
        "raynkami/rust.auto:${TAG != "" ? TAG : "v0.0.34"}",
        "raynkami/rust.auto:latest"
    ]
}
