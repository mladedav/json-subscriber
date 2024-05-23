// #[derive(Default)]
// pub struct SubscriberBuilder {
// }

// impl<N, E, F, W> SubscriberBuilder<N, E, F, W>
// where
//     N: for<'writer> FormatFields<'writer> + 'static,
//     E: FormatEvent<Registry, N> + 'static,
//     W: for<'writer> MakeWriter<'writer> + 'static,
//     F: subscribe::Subscribe<Formatter<N, E, W>> + Send + Sync + 'static,
//     fmt_subscriber::Subscriber<Registry, N, E, W>:
//         subscribe::Subscribe<Registry> + Send + Sync + 'static,
// {
//     /// Finish the builder, returning a new `FmtCollector`.
//     #[must_use = "you may want to use `try_init` or similar to actually install the collector."]
//     pub fn finish(self) -> Collector<N, E, F, W> {
//         let collector = self.inner.with_collector(Registry::default());
//         Collector {
//             inner: self.filter.with_collector(collector),
//         }
//     }

//     /// Install this collector as the global default if one is
//     /// not already set.
//     ///
//     /// If the `tracing-log` feature is enabled, this will also install
//     /// the LogTracer to convert `Log` records into `tracing` `Event`s.
//     ///
//     /// # Errors
//     /// Returns an Error if the initialization was unsuccessful, likely
//     /// because a global collector was already installed by another
//     /// call to `try_init`.
//     pub fn try_init(self) -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
//         use crate::util::SubscriberInitExt;
//         self.finish().try_init()?;

//         Ok(())
//     }

//     /// Install this collector as the global default.
//     ///
//     /// If the `tracing-log` feature is enabled, this will also install
//     /// the LogTracer to convert `Log` records into `tracing` `Event`s.
//     ///
//     /// # Panics
//     /// Panics if the initialization was unsuccessful, likely because a
//     /// global collector was already installed by another call to `try_init`.
//     pub fn init(self) {
//         self.try_init().expect("Unable to install global collector")
//     }
// }
