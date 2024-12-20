---
seotitle: Create recurring tasks with Encore's Cron Jobs API
seodesc: Learn how to create periodic and recurring tasks in your backend application using Encore's Cron Jobs API.
title: Cron Jobs
subtitle: Run recurring and scheduled tasks
infobox: {
  title: "Cron Jobs",
  import: "encore.dev/cron",
  example_link: "/docs/tutorials/uptime"
}
lang: go
---

When you need to run periodic and recurring tasks, Encore.go provides a declarative way of using Cron Jobs.

When a Cron Job is defined in your application, Encore automatically calls your specified API according to the defined schedule. This eliminates the need for infrastructure maintenance, as Encore manages scheduling, monitoring, and execution of Cron Jobs.

<Callout type="info">

Cron Jobs do not run when developing locally or in [Preview Environments](/docs/platform/deploy/preview-environments), but you can always call the API manually to test the behavior.

</Callout>

<GitHubLink
    href="https://github.com/encoredev/examples/tree/main/uptime"
    desc="Uptime Monitoring app that uses a Cron Job to periodically check the uptime of a website."
/>

## Defining a Cron Job

To define a Cron Job, import the `encore.dev/cron` [package](https://pkg.go.dev/encore.dev/cron),
and call the `cron.NewJob()` function and store it as a package-level variable.

### Example

```go
import "encore.dev/cron"

// Send a welcome email to everyone who signed up in the last two hours.
var _ = cron.NewJob("welcome-email", cron.JobConfig{
	Title:    "Send welcome emails",
	Every:    2 * cron.Hour,
	Endpoint: SendWelcomeEmail,
})

// SendWelcomeEmail emails everyone who signed up recently.
// It's idempotent: it only sends a welcome email to each person once.
//encore:api private
func SendWelcomeEmail(ctx context.Context) error {
	// ...
	return nil
}
```

The `"welcome-email"` argument to `cron.NewJob` is a unique ID you give to each Cron Job.
If you later refactor the code and move the Cron Job definition to another package,
we use this ID to keep track that it's the same Cron Job and not a different one.

When this code gets deployed Encore will automatically register the Cron Job in Encore Cloud
and begin calling the `SendWelcomeEmail` API every hour.

The Encore Cloud dashboard provides a convenient user interface for monitoring and debugging
Cron Job executions across all your environments via the `Cron Jobs` menu item:

![Cron Jobs UI](/assets/docs/cron.png)

## Keep in mind when using Cron Jobs

- Cron Jobs do not execute during local development or in [Preview Environments](/docs/platform/deploy/preview-environments). However, you can manually invoke the API to test its behavior.
- In Encore Cloud, Cron Job executions are limited to **once every hour**, with the exact minute randomized within that hour for users on the Free Tier. To enable more frequent executions or to specify the exact minute within the hour, consider [deploying to your own cloud](/docs/platform/deploy/own-cloud) or upgrading to the [Pro plan](/pricing).
- Both public and private APIs are supported for Cron Jobs.
- Ensure that the API endpoints used in Cron Jobs are idempotent, as they may be called multiple times under certain network conditions.
- The API endpoints used in Cron Jobs must not take any request parameters. That is, their signatures must be `func(context.Context) error` or `func(context.Context) (*T, error)`.

## Cron schedules

Above we used the `Every` field, which executes the Cron Job on a periodic basis.
It runs around the clock each day, starting at midnight (UTC).

In order to ensure a consistent delay between each run, the interval used **must divide 24 hours evenly**.
For example, `10 * cron.Minute` and `6 * cron.Hour` are both allowed (since 24 hours is evenly divisible by both),
whereas `7 * cron.Hour` is not (since 24 is not evenly divisible by 7).
The Encore compiler will catch this and give you a helpful error at compile-time if you try to use an invalid interval.

### Cron expressions

For more advanced use cases, such as running a Cron Job on a specific day of the month, or a specific week day, or similar,
the `Every` field is not expressive enough.

For these use cases, Encore provides full support for [Cron expressions](https://en.wikipedia.org/wiki/Cron) by using the `Schedule` field
instead of the `Every` field.

Cron expressions allow you to define precise schedules for your tasks, including specific days of the week, specific hours of the day, and more. Note that all times are expressed in UTC.

For example:

```go
// Run the monthly accounting sync job at 4am (UTC) on the 15th day of each month.
var _ = cron.NewJob("accounting-sync", cron.JobConfig{
	Title:    "Cron Job Example",
	Schedule: "0 4 15 * *",
	Endpoint: AccountingSync,
})
```
