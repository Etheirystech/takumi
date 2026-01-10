import { HomeLayout } from "fumadocs-ui/layouts/home";
import { Link } from "react-router";
import { Button } from "~/components/ui/button";
import { baseOptions } from "~/layout-config";
import { showcaseFeatures } from "../data/showcase";

export default function Home() {
  return (
    <HomeLayout className="text-center" {...baseOptions}>
      <title>Takumi makes dynamic image rendering simple.</title>
      <meta
        name="description"
        content="Production-ready library to make rendering performant, portable and scalable."
      />
      <meta
        name="og:title"
        content="Takumi makes dynamic image rendering simple"
      />
      <meta
        name="og:description"
        content="Production-ready library to make rendering performant, portable and scalable."
      />
      <meta
        name="og:image"
        content="https://raw.githubusercontent.com/kane50613/takumi/master/example/twitter-images/output/og-image.png"
      />
      <meta
        name="twitter:image"
        content="https://raw.githubusercontent.com/kane50613/takumi/master/example/twitter-images/output/og-image.png"
      />
      <div className="max-w-7xl w-full mx-auto px-4">
        <div className="flex flex-col py-24 items-center justify-center">
          <img src="/logo.svg" className="w-16 h-auto" alt="Takumi Logo" />
          <h1 className="py-6 text-3xl sm:text-5xl font-semibold max-w-4xl text-balance">
            <span className="text-primary">Takumi</span> makes dynamic image
            rendering simple.
          </h1>
          <p className="text-muted-foreground text-base sm:text-lg max-w-md mb-8">
            Production-ready library to make rendering performant, portable and
            scalable.
          </p>
          <div className="flex gap-2.5 mb-24">
            <Button asChild className="rounded-full" size="lg">
              <Link to="/docs">Open Docs</Link>
            </Button>
            <Button
              asChild
              className="rounded-full"
              variant="outline"
              size="lg"
            >
              <Link to="/playground">Try in Playground</Link>
            </Button>
          </div>
        </div>

        <section className="mb-24 text-left">
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-12 items-center">
            <div>
              <h2 className="text-3xl font-bold mb-6">Unmatched Features</h2>
              <div className="space-y-6">
                {showcaseFeatures.map((feature) => (
                  <div key={feature.title} className="flex gap-4">
                    <div className="mt-1 p-2 rounded-lg bg-primary/10 text-primary shrink-0 h-fit">
                      <feature.icon className="w-5 h-5" />
                    </div>
                    <div>
                      <h4 className="font-bold mb-1 border-b inline-block border-primary/20">
                        {feature.title}
                      </h4>
                      <p className="text-muted-foreground text-sm">
                        {feature.description}
                      </p>
                    </div>
                  </div>
                ))}
              </div>
            </div>
            <div className="relative group">
              <div className="absolute -inset-4 bg-linear-to-r from-primary/20 to-blue-500/20 blur-3xl opacity-50 group-hover:opacity-75 transition-opacity" />
              <div className="relative rounded-3xl border bg-card/50 backdrop-blur-sm p-2 shadow-2xl overflow-hidden">
                <img
                  src="https://raw.githubusercontent.com/kane50613/takumi/refs/heads/master/example/twitter-images/output/x-post-image.png"
                  alt="X.com Post opengraph"
                  className="rounded-2xl"
                />
              </div>
            </div>
          </div>
        </section>
      </div>
    </HomeLayout>
  );
}
