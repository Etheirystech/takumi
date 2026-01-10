import { SiGithub } from "@icons-pack/react-simple-icons";
import { HomeLayout } from "fumadocs-ui/layouts/home";
import JSConfetti from "js-confetti";
import { Heart, LayoutTemplate, Link2Icon } from "lucide-react";
import { useCallback, useMemo, useRef } from "react";
import { baseOptions } from "~/layout-config";
import {
  type Project,
  showcaseProjects,
  showcaseTemplates,
} from "../data/showcase";

function Card({ project }: { project: Project }) {
  const title = useMemo(() => {
    if (!project.title) {
      const { hostname, pathname } = new URL(project.url);

      if (hostname === "github.com") {
        const [owner, repo] = pathname.split("/").filter(Boolean);

        return `${owner}/${repo}`;
      }

      return hostname;
    }

    return project.title;
  }, [project.title, project.url]);

  const icon = useMemo(() => {
    if (project.url.includes("github.com")) {
      return <SiGithub size={18} />;
    }

    return <Link2Icon size={18} />;
  }, [project.url]);

  return (
    <a
      href={project.url}
      target="_blank"
      rel="noopener noreferrer"
      className="border rounded-lg overflow-hidden group bg-muted/10"
    >
      <div className="relative aspect-1200/630 overflow-hidden bg-muted/30">
        <img
          src={project.image}
          alt="Blur background"
          className="absolute inset-0 w-full h-full object-cover blur-xs scale-110 opacity-75 select-none pointer-events-none"
          width={project.width}
          height={project.height}
          loading="lazy"
          decoding="async"
        />
        <img
          src={project.image}
          alt={title}
          className="relative w-full h-full object-contain transition-transform duration-500 group-hover:scale-[1.02]"
          width={project.width}
          height={project.height}
          loading="lazy"
          decoding="async"
        />
      </div>
      <div className="px-4 py-2 border-t flex items-center gap-2 text-foreground/80 group-hover:text-foreground transition-colors duration-300">
        {icon}
        <span className="text-sm font-medium">{title}</span>
      </div>
    </a>
  );
}

export default function Showcase() {
  const confettiRef = useRef<JSConfetti | null>(null);

  const onConfetti = useCallback((e: React.MouseEvent<HTMLButtonElement>) => {
    if (!confettiRef.current) {
      confettiRef.current = new JSConfetti();
    }

    const rect = e.currentTarget.getBoundingClientRect();
    const x = rect.left + rect.width / 2;
    const y = rect.top + rect.height / 2;

    confettiRef.current.addConfettiAtPosition({
      emojis: ["❤️", "🪓"],
      emojiSize: 50,
      confettiNumber: 25,
      confettiDispatchPosition: { x, y },
    });
  }, []);

  return (
    <HomeLayout {...baseOptions}>
      <title>Showcase</title>
      <meta
        name="description"
        content="Discover how developers are using Takumi to power their dynamic image generation."
      />
      <div className="container py-24 px-4 mx-auto max-w-8xl">
        <div className="flex flex-col items-center text-center mb-16">
          <div className="relative mb-6">
            <div className="absolute -inset-4 bg-primary/20 blur-2xl rounded-full animate-pulse duration-300" />
            <button
              type="button"
              onClick={onConfetti}
              className="relative group transition-transform active:scale-95 cursor-pointer outline-none"
            >
              <Heart className="w-16 h-16 text-primary fill-primary drop-shadow-[0_0_15px_rgba(239,68,68,0.5)] transition-transform group-hover:scale-110 duration-300" />
            </button>
          </div>
          <h1 className="text-4xl md:text-6xl font-bold mb-6 tracking-tight">
            Built with <span className="text-primary">Takumi</span>
          </h1>
          <p className="text-muted-foreground text-lg md:text-xl max-w-2xl mx-auto text-pretty">
            Discover how developers are using Takumi to power their dynamic
            image generation.
          </p>
        </div>

        <section className="mb-24 grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          {showcaseProjects.map((project) => (
            <Card key={project.url} project={project} />
          ))}
        </section>

        <section className="mb-24">
          <h2 className="text-2xl font-semibold mb-8 flex items-center gap-2">
            <LayoutTemplate className="w-6 h-6 text-blue-500" />
            Ready-to-use Templates
          </h2>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-6">
            {showcaseTemplates.map((item) => (
              <a
                key={item.title}
                href={item.href}
                className="group flex flex-col space-y-4"
              >
                <div className="relative aspect-1200/630 overflow-hidden rounded-lg bg-muted/30 border">
                  <img
                    src={item.image}
                    alt=""
                    className="absolute inset-0 w-full h-full object-cover blur-2xl scale-110 opacity-50 select-none pointer-events-none"
                  />
                  <img
                    src={item.image}
                    alt={`${item.title} layout example`}
                    width={1200}
                    height={630}
                    className="relative w-full h-full object-contain transition-transform duration-700 group-hover:scale-105"
                  />
                </div>
                <h3 className="font-semibold text-lg inline-flex items-center gap-2">
                  {item.title} Template
                  <span className="text-primary group-hover:translate-x-1 transition-transform">
                    &rarr;
                  </span>
                </h3>
              </a>
            ))}
          </div>
        </section>

        <div className="rounded-3xl bg-primary/80 p-8 md:p-16 text-primary-foreground text-center relative overflow-hidden">
          <div className="absolute top-0 left-0 w-full h-full bg-[radial-gradient(circle_at_30%_50%,rgba(255,255,255,0.1),transparent)]" />
          <div className="relative z-10">
            <h2 className="text-3xl md:text-4xl font-bold mb-4">
              Want to be featured?
            </h2>
            <p className="text-primary-foreground/80 mb-8 max-w-xl mx-auto">
              Built something cool with Takumi? We&apos;d love to show it off!
              <br />
              Submit your project to our GitHub repository.
            </p>
            <div className="flex flex-wrap justify-center gap-4">
              <a
                href="https://github.com/kane50613/takumi/edit/master/docs/app/data/showcase.ts"
                target="_blank"
                rel="noreferrer"
                className="bg-white text-primary px-8 py-3 rounded-full font-medium hover:shadow-lg transition-transform hover:-translate-y-1"
              >
                Make a Pull Request
              </a>
            </div>
          </div>
        </div>
      </div>
    </HomeLayout>
  );
}
