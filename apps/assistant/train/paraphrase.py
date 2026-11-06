"""New wordings for gen.py's phrase lists, from a local LLM on the GPU machine.

    python -I paraphrase.py MODEL_DIR BANKS.json OUT.json [--rounds N] [--seed N]

BANKS.json is `gen.py --banks`: each list's phrasings, what they all mean, and
a yes-or-no check. For each list the model is asked, a few times with a
different handful of examples, for more ways a person would type the same
thing; each new phrasing is then put to the check, and kept only when the model
answers as the list wants. Labels never come from the model: a phrasing joins
the list it was asked for, and gen.py gives it that list's answer.

It runs offline (vLLM in this process, the weights in MODEL_DIR, no server, no
port) and writes {key: [phrasing, ...]} to OUT.json.
"""

import json
import random
import re
import sys

SLOTS = {"{x}": "bluetooth", "{app}": "Firefox", "{n}": "40"}

SYSTEM = ("You write the short things people type or say to their computer's assistant. "
          "You answer with the phrasings only, one per line.")


def ask(meaning, examples, count):
    slots = [s for s in SLOTS if s in meaning or any(s in e for e in examples)]
    keep = (f" Every line must contain {' and '.join(slots)} exactly as written, as a "
            "placeholder." if slots else "")
    shown = "\n".join(examples)
    return (f"Each of these is someone {meaning}:\n\n{shown}\n\n"
            f"Write {count} more that mean the same, each different from these and from each "
            "other. Mix it up the way real people do: terse, chatty, indirect, a complaint, "
            "a reason, slang, now and then no verb at all. Lowercase, no numbering, no quotes, "
            f"at most 14 words each.{keep}")


def lines(text, slots):
    out = []
    for line in text.splitlines():
        line = re.sub(r"^\s*(?:[-*•]|\d+[.)])\s*", "", line).strip().strip('"\'').strip()
        line = line.lower().replace("\u2019", "'").replace("\u2018", "'")
        if not line or len(line.split()) > 14 or len(line) > 100:
            continue
        # the placeholders as asked, and no others
        found = set(re.findall(r"\{[a-z]+\}", line))
        if found != slots or re.sub(r"\{[a-z]+\}", "", line).count("{") or "}" in re.sub(r"\{[a-z]+\}", "", line):
            continue
        out.append(line)
    return out


def plain(q):
    return " ".join(re.sub(r"[^a-z0-9%{} ]", " ", q.lower().replace("'", "")).split())


def main():
    args = sys.argv[1:]

    def option(name, default):
        if name in args:
            i = args.index(name)
            value = args[i + 1]
            del args[i:i + 2]
            return value
        return default

    rounds = int(option("--rounds", "6"))
    seed = int(option("--seed", "0"))
    model_dir, banks_path, out_path = args
    rng = random.Random(seed)
    with open(banks_path) as f:
        banks = json.load(f)

    from vllm import LLM, SamplingParams

    # eager: no graph compiling, which would need a compiler toolchain on the machine
    llm = LLM(model=model_dir, max_model_len=4096, gpu_memory_utilization=0.85, seed=seed,
              enforce_eager=True)
    chat = {"chat_template_kwargs": {"enable_thinking": False}}

    def say(prompt):
        return [{"role": "system", "content": SYSTEM}, {"role": "user", "content": prompt}]

    # 1. new phrasings: the lists every machine uses (a named setting, an app,
    #    what isn't a command) are asked twice as often
    asks, owners = [], []
    for bank in banks:
        if not bank["phrases"]:
            continue
        wide = any(s in bank["meaning"] for s in SLOTS) or bank["key"] == "none/general"
        for _ in range(rounds * (2 if wide else 1)):
            examples = rng.sample(bank["phrases"], min(10, len(bank["phrases"])))
            asks.append(say(ask(bank["meaning"], examples, 15)))
            owners.append(bank)
    print(f"asking for phrasings: {len(asks)} prompts over {len(banks)} lists", flush=True)
    made = llm.chat(asks, SamplingParams(temperature=1.0, top_p=0.95, max_tokens=700, seed=seed),
                    use_tqdm=False, **chat)

    candidates = {}
    for bank, result in zip(owners, made):
        slots = {s for s in SLOTS if s in bank["meaning"]}
        have = candidates.setdefault(bank["key"], {plain(p): None for p in bank["phrases"]})
        for line in lines(result.outputs[0].text, slots):
            have.setdefault(plain(line), line)
    fresh = [(key, line) for key, have in candidates.items() for line in have.values() if line]
    print(f"made {len(fresh)} new phrasings", flush=True)

    # 2. the check: each, with its placeholders filled in, against what its list means
    by_key = {bank["key"]: bank for bank in banks}

    def filled(line):
        for slot, value in SLOTS.items():
            line = line.replace(slot, value)
        return line

    checks = [say(f'Someone typed this to their computer\'s assistant: "{filled(line)}"\n\n'
                  f'{by_key[key]["check"]} Answer only yes or no.')
              for key, line in fresh]
    answers = llm.chat(checks, SamplingParams(temperature=0.0, max_tokens=4), use_tqdm=False, **chat)

    kept, refused = {}, 0
    for (key, line), answer in zip(fresh, answers):
        said = answer.outputs[0].text.strip().lower()
        if said.startswith(by_key[key]["want"]):
            kept.setdefault(key, []).append(line)
        else:
            refused += 1
    with open(out_path, "w") as f:
        json.dump(kept, f, indent=1, ensure_ascii=False, sort_keys=True)
    print(f"kept {sum(map(len, kept.values()))} phrasings, {refused} failed the check -> {out_path}",
          flush=True)


if __name__ == "__main__":
    main()
