# Licensing

The server is licensed [AGPL-3.0-only](../LICENSE), and that is the whole
of the offer. Contributions are signed off under the Developer
Certificate of Origin; contributors keep their copyright.

This page answers the questions an adoption committee actually asks, in
plain English, before they have to be asked somewhere we cannot reply. It
is an engineering summary of what the licence says — not legal advice, and
not a modification of it. See the closing note.

## If we run it, do we have to publish anything?

No.

Running the server as distributed — internally, or as a public service, at
any scale, including one you charge for — creates no obligation to publish
anything. Keep the copyright and licence notices intact and you are done.
Use does not trigger the AGPL. Nothing about serving the public triggers
it either.

## What if we modify it?

Then one obligation attaches, and only one. Section 13 says that if you
modify the Program and your modified version interacts with users remotely
over a network, it must offer those users an opportunity to receive the
Corresponding Source of your version.

Three things that clause does not say, listed because they are what people
assume it says:

- **Modification alone does not trigger it.** Modify it, run it privately,
  never expose the modified version over a network, and nothing is owed.
- **It is the source of the modified server, not of your stack.** Your
  catalogue, ingest pipeline, storage layer and infrastructure are not the
  Program and are not covered by it.
- **The offer runs to the users of your service** — not to the world, not
  to a public repository, and not to us.

## Does this affect our viewer, manifests, or discovery layer?

No. This is usually the question underneath the other questions, and it is
the one where the fear is most misplaced.

Those systems talk to the server over HTTP, as separate processes. They
are not linked into it, do not incorporate it, and are not derivative works
of it — copyleft does not propagate across a network protocol between
separate programs. Your OpenSeadragon or Mirador front end, your
Presentation API manifests, your catalogue and your discovery layer keep
whatever licences they already have, proprietary included.

The architecture makes this concrete rather than merely arguable. This
engine is a pixel-serving box and nothing else: pre-refusals in
[MAINTENANCE.md](../MAINTENANCE.md) rule out auth, the Presentation API,
manifests and viewers permanently. Your application sends a IIIF URL and
receives an image. That is the entire coupling.

## We want to ship it inside a product

Distributing the software — in an appliance, an installer, an on-premise
product — is conveying, and the AGPL's source obligations apply to what you
convey. If that does not fit how you ship, the next question is for you.

## Can we get it under different terms?

**No — and this answer changed at the import (2026-08-21).**

Until then the project carried an Apache-ICLA-derived contributor
agreement whose §2 granted the maintainer the right to license
contributions "under any license terms the Maintainer chooses ...
including proprietary and commercial license terms". That grant is what
made alternative terms real rather than hypothetical, and it is gone.

The organisation this project now belongs to attests contributions with
the [Developer Certificate of Origin](https://developercertificate.org/)
and operates no contributor licence agreement. Under a DCO sign-off a
contributor asserts provenance and grants nothing beyond the project's
own licence, so nobody holds the right to relicense contributed code.
Offering the tree under other terms would need the agreement of every
copyright holder in it.

Two things follow, and both are worth stating plainly rather than
leaving to be discovered:

- If you were counting on a commercial licence being available on
  request, it is not. Plan against the AGPL as written.
- The rug-pull commitment that rode in the same clause — "The Project
  itself will always remain available under its open-source license" —
  is no longer a contractual promise either. What replaces it is
  structural, and arguably stronger: with no relicensing grant, nobody
  *can* take it closed. The licence on the tree is the only offer there
  is, and for code already published under it that grant is
  irrevocable.

## Why AGPL, rather than a permissive licence?

Deliberate, and worth stating once here rather than defending case by case.
The established alternative in this space is permissively licensed, so the
burden of explanation sits with us.

- **Improvements to a shared implementation of a public specification
  should stay available to the people running it.** For server software the
  GPL alone does not achieve that, because deploying a service is not
  distributing software. The network clause is the part that closes it.
- **The relicensing grant is what lets alternative terms fund maintenance
  of deliberately scope-frozen software.** The commitment in
  [MAINTENANCE.md](../MAINTENANCE.md) is security and correctness fixes
  forever, which is a real cost with no feature roadmap to sell against. A
  permissive licence forecloses that funding route, and forecloses it
  permanently.

## Our institution has a blanket AGPL policy

Some do — applied to a category rather than to facts, and often written
before AGPL server software was common. Evaluations lost that way are lost
to a policy rather than to a technical comparison, and that is an accepted
cost of the position above, not something this page can argue away.

If the policy has an exception process, the answers above are the evidence
it will want; the viewer question is usually the one that resolves it. If
it does not, alternative terms exist.

## Machine-readable licensing

The repository is [REUSE 3.3](https://reuse.software/spec-3.3/) compliant
and CI enforces it on every push, so every single file carries its
copyright and licence information. A scanner in your procurement pipeline
can answer the per-file question without a human reading anything, and the
[REUSE API](https://api.reuse.software/info/github.com/monumental-archive/iiif-server)
reports the current state independently of any claim made here.

## Not legal advice

Everything above is an engineering summary of intent, written by the
maintainer and not by a lawyer. It does not modify the licence: where this
page and [LICENSE](../LICENSE) disagree, the licence governs and this page
is wrong. Your counsel should read the licence. Anything commercial will
want lawyers on both sides.
