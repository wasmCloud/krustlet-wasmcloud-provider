**Deprecation Notice**: After much discussion among maintainers, we have decided to deprecate this
repo in favor of the new [wasmCloud Helm Chart](https://github.com/wasmCloud/wasmcloud-otp/tree/main/wasmcloud_host/chart).
We arrived at this decision due to 2 main reasons. First, the amount of work to bring this up to date
with both Krustlet 1.0 and the shiny new 0.50+ wasmCloud host is more than we are able to commit
to at this time. More importantly, running your wasmCloud actors on wasmCloud-specific nodes severely
limits the rich feature set wasmCloud provides. To that end we created the Helm chart, which allows
you to run and easily scale wasmCloud hosts running as Kubernetes pods. This enables integration
with existing services running in Kubernetes while not hampering the power of wasmCloud.

# wasmCloud Krustlet provider

This is a [Krustlet](https://github.com/deislabs/krustlet)
[Provider](https://github.com/deislabs/krustlet/blob/master/docs/topics/architecture.md#providers)
implementation for the [wasmCloud](https://github.com/wasmCloud/wasmCloud) runtime.

## Documentation

If you're new to Krustlet, get started with [the
introduction](https://github.com/deislabs/krustlet/blob/master/docs/intro/README.md) documentation.
For more in-depth information about Krustlet, plunge right into the [topic
guides](https://github.com/deislabs/krustlet/blob/master/docs/topics/README.md).

## Community, discussion, contribution, and support

You can reach the Krustlet community and developers via the following channels:

- [Kubernetes Slack](https://kubernetes.slack.com):
  - [#krustlet](https://kubernetes.slack.com/messages/krustlet)
- Public Community Call on Mondays at 1:00 PM PT:
  - [Zoom](https://us04web.zoom.us/j/71695031152?pwd=T0g1d0JDZVdiMHpNNVF1blhxVC9qUT09)
  - Download the meeting calendar invite
    [here](https://raw.githubusercontent.com/deislabs/krustlet/master/docs/community/assets/community_meeting.ics)
