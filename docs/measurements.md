# Measurements

What the engine's builds and benchmarks measured, one section per milestone or machine, newest last
([ADR-0104](adr/0104-the-build-ci-and-measurements.md)). Each section names its commit, its date and
its machine, which a figure compares with only: the same benchmark on the same machine, driver and
build. A change that claims a gain shows the section before it and the section after it.

[`scripts/measure.sh`](../scripts/measure.sh) writes a section:

```sh
VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" scripts/measure.sh "<milestone>, lavapipe" >> docs/measurements.md
scripts/measure.sh "<milestone>, <GPU>" >> docs/measurements.md                # on a machine's own GPU
```

It measures the build of the workspace and of its tests from an empty target directory, with the
default debug settings, the run of the workspace's tests, and the benchmarks of
[`slime-benches`](../benches/README.md), which says what each scenario is and how its figures are
read. The figures that the plan's known debt and the ADRs give were measured once; this file holds
those that are measured again.
