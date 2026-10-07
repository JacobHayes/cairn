# Entity creates in another domain's patch advance the deployment revision

- Question: E6 lets an entity create ride in any patch with no deployment revision check, and asks a journey patch that writes an entity reference to name the deployment revision it was validated against. Whether a create riding in a journey patch advances the deployment revision is left open, and the fixtures need an answer.
- Call: yes, once per patch that creates entities (it writes deployment records, and A17's revision counts a domain's changes). A later patch referring to those entities names the resulting revision; a patch referring only to entities it creates itself names none. Each fixture scenario runs in a fresh deployment, and a fixture test checks the rule.
- Alternatives: only deployment-domain patches advance the deployment revision (then a reference check could miss a create that landed in between).
- What would change it: the engine or store (2.1, 3.1) committing riding creates without a deployment revision bump.
