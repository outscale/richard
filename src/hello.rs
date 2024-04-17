use crate::bot::{
    Message, MessageCtx, MessageResponse, Module, ModuleCapabilities, ModuleData, ModuleParam,
};
use async_trait::async_trait;
use rand::prelude::IteratorRandom;
use std::env::VarError;
use tokio::time::Duration;

#[async_trait]
impl Module for Hello {
    fn name(&self) -> &'static str {
        "hello"
    }

    fn params(&self) -> Vec<ModuleParam> {
        Vec::new()
    }

    async fn module_offering(&mut self, _modules: &[ModuleData]) {}

    async fn run(&mut self, _variation: usize) -> Option<Vec<Message>> {
        if !self.has_skipped_first_time {
            self.has_skipped_first_time = true;
            return None;
        }
        let quote = {
            let mut rng = rand::thread_rng();
            match ALL_QUOTES.iter().choose(&mut rng) {
                Some((author, quote)) => format!("{} — {}", quote, author),
                None => return None,
            }
        };
        Some(vec![quote])
    }

    fn capabilities(&self) -> ModuleCapabilities {
        ModuleCapabilities::default()
    }

    async fn variation_durations(&mut self) -> Vec<Duration> {
        let seven_day_s = 7 * 24 * 60 * 60;
        vec![Duration::from_secs(seven_day_s)]
    }

    async fn trigger(&mut self, _message: &str) -> Option<Vec<MessageResponse>> {
        None
    }

    async fn send_message(&mut self, _messages: &[Message]) {}

    async fn read_message(&mut self) -> Option<Vec<MessageCtx>> {
        None
    }

    async fn resp_message(&mut self, _parent: MessageCtx, _message: Message) {}
}

#[derive(Clone)]
pub struct Hello {
    has_skipped_first_time: bool,
}

impl Hello {
    pub fn new() -> Result<Self, VarError> {
        Ok(Hello {
            has_skipped_first_time: false,
        })
    }
}

const ALL_QUOTES: [(&str, &str); 55] = [
    // The following quotes are from https://en.wikiquote.org/  Creative Commons Attribution-ShareAlike License
    ("Linus Torvalds", "I'm doing a (free) operating system (just a hobby, won't be big and professional like gnu) for 386(486) AT clones."),
    ("Linus Torvalds", "Making Linux GPL'd was definitely the best thing I ever did."),
    ("Linus Torvalds", "Talk is cheap. Show me the code."),
    ("Linus Torvalds", "For example, the GPLv2 in no way limits your use of the software. If you're a mad scientist, you can use GPLv2'd software for your evil plans to take over the world (\"Sharks with lasers on their heads!!\"), and the GPLv2 just says that you have to give source code back. And that's OK by me. I like sharks with lasers. I just want the mad scientists of the world to pay me back in kind. I made source code available to them, they have to make their changes to it available to me. After that, they can fry me with their shark-mounted lasers all they want."),
    ("Linus Torvalds", "I am a lazy person, which is why I like open source, for other people to do work for me."),
    ("John D. Carmack", "Sharing the code just seems like The Right Thing to Do, it costs us rather little, but it benefits a lot of people in sometimes very significant ways."),
    ("Richard M. Stallman", "GNU, which stands for Gnu's Not Unix, is the name for the complete Unix-compatible software system which I am writing so that I can give it away free to everyone who can use it."),
    ("Richard M. Stallman", "To avoid horrible confusion, please pronounce the G in the word GNU when it is the name of this project."),
    ("Richard M. Stallman", "I consider that the golden rule requires that if I like a program I must share it with other people who like it."),
    ("Richard M. Stallman", "GNU is not in the public domain. Everyone will be permitted to modify and redistribute GNU, but no distributor will be allowed to restrict its further redistribution."),
    ("Richard M. Stallman", "Once GNU is written, everyone will be able to obtain good system software free, just like air."),
    ("Richard M. Stallman", "A hacker is someone who enjoys playful cleverness — not necessarily with computers."),
    ("Richard M. Stallman", "The use of “hacker” to mean “security breaker” is a confusion on the part of the mass media. We hackers refuse to recognize that meaning, and continue using the word to mean someone who loves to program, someone who enjoys playful cleverness, or the combination of the two."),
    ("Richard M. Stallman", "If we are content with knowledge as a commodity, accessible only through a computerized bureaucracy, we can simply let companies provide it. But if we want to keep human knowledge open and freely available to humanity, we have to do the work to make it available that way. We have to write a free encyclopedia."),
    ("Richard M. Stallman", "Very ironic things have happened, but nothing to match this — giving the Linus Torvalds Award to the Free Software Foundation is sort of like giving the Han Solo Award to the Rebel Fleet."),
    ("Richard M. Stallman", "Every decision a person makes stems from the person's values and goals. People can have many different goals and values; fame, profit, love, survival, fun, and freedom, are just some of the goals that a good person might have. When the goal is to help others as well as oneself, we call that idealism. My work on free software is motivated by an idealistic goal: spreading freedom and cooperation. I want to encourage free software to spread, replacing proprietary software that forbids cooperation, and thus make our society better."),
    ("Richard M. Stallman", "While free software by any other name would give you the same freedom, it makes a big difference which name we use: different words convey different ideas."),
    ("Richard M. Stallman", "We are not against the Open Source movement, but we don't want to be lumped in with them. We acknowledge that they have contributed to our community, but we created this community, and we want people to know this."),
    ("Richard M. Stallman", "The term \"free software\" has an ambiguity problem: an unintended meaning, \"Software you can get for zero price,\" fits the term just as well as the intended meaning, \"software which gives the user certain freedoms.\" We address this problem by publishing a more precise definition of free software, but this is not a perfect solution; it cannot completely eliminate the problem. An unambiguously correct term would be better, if it didn't have other problems."),
    ("Richard M. Stallman", "The official definition of \"open source software,\" as published by the Open Source Initiative, is very close to our definition of free software; however, it is a little looser in some respects, and they have accepted a few licenses that we consider unacceptably restrictive of the users."),
    ("Richard M. Stallman", "Value your freedom or you will lose it, teaches history. \"Don't bother us with politics,\" respond those who don't want to learn."),
    ("Richard M. Stallman", "Geeks like to think that they can ignore politics, you can leave politics alone, but politics won't leave you alone."),
    ("Richard M. Stallman", "Thanks to Mr. Gates, we now know that an open Internet with protocols anyone can implement is communism; it was set up by that famous communist agent, the US Department of Defense."),
    ("Richard M. Stallman", "People said I should accept the world. Bullshit! I don't accept the world."),
    ("Richard M. Stallman", "You can use any editor you want, but remember that vi vi vi is the text editor of the beast."),
    ("Richard M. Stallman", "For personal reasons, I do not browse the web from my computer."),
    ("Richard M. Stallman", "I have to explain that I'm not an anarchist – I have a pro-state gland."),
    ("Richard M. Stallman", "Isn't it ironic that the proprietary software developers call us communists? We are the ones who have provided for a free market, where they allow only monopoly."),
    ("Richard M. Stallman", "The GNU GPL was not designed to be \"open source\". I wrote it for the free software movement, and its purpose is to ensure every user of every version of the program gets the essential freedoms. See http://www.gnu.org/philosophy/open-source-misses-the-point.html for more explanation of the difference between free software and open source."),
    ("Richard M. Stallman", "For personal reasons, I do not browse the web from my computer. (I also have no net connection much of the time.) To look at page I send mail to a daemon which runs wget and mails the page back to me. It is very efficient use of my time, but it is slow in real time."),
    ("Richard M. Stallman", "Corporations don't have to be decent. Real persons, if they do something that's lawful but nasty you'll say 'you are a jerk, you are acting like a jerk, stop it!'. But we are not supposed to ever say that to these phony people. We are supposed to say 'oh well, it's lawful so we'll just have to suffer it'."),
    ("Richard M. Stallman", "Nobody deserves to have to die — not Jobs, not Mr. Bill, not even people guilty of bigger evils than theirs. But we all deserve the end of Jobs' malign influence on people's computing."),
    ("Richard M. Stallman", "It doesn't take special talents to reproduce — even plants can do it. On the other hand, contributing to a program like Emacs takes real skill. That is really something to be proud of. It helps more people, too."),
    ("Richard M. Stallman", "I am a pessimist by nature. Many people can only keep on fighting when they expect to win. I'm not like that, I always expect to lose. I fight anyway, and sometimes I win."),
    ("Richard M. Stallman", "Fighting patents one by one will never eliminate the danger of software patents, any more than swatting mosquitoes will eliminate malaria."),
    ("Richard M. Stallman", "Free software permits students to learn how software works. Some students, on reaching their teens, want to learn everything there is to know about their computer and its software. They are intensely curious to read the source code of the programs that they use every day. To learn to write good code, students need to read lots of code and write lots of code. They need to read and understand real programs that people really use. Only free software permits this. Proprietary software rejects their thirst for knowledge: it says, “The knowledge you want is a secret — learning is forbidden!” Free software encourages everyone to learn. The free software community rejects the “priesthood of technology”, which keeps the general public in ignorance of how technology works; we encourage students of any age and situation to read the source code and learn as much as they want to know. Schools that use free software will enable gifted programming students to advance."),
    ("Richard M. Stallman", "It is hard to write a simple definition of something as varied as hacking, but I think what these activities have in common is playfulness, cleverness, and exploration. Thus, hacking means exploring the limits of what is possible, in a spirit of playful cleverness. Activities that display playful cleverness have 'hack value'."),
    ("Richard M. Stallman", "People sometimes ask me if it is a sin in the Church of Emacs to use vi. Using a free version of vi is not a sin; it is a penance. So happy hacking."),
    ("Richard M. Stallman", "I could have made money this way, and perhaps amused myself writing code. But I knew that at the end of my career, I would look back on years of building walls to divide people, and feel I had spent my life making the world a worse place."),
    ("Richard M. Stallman", "Stallman's Law (2012): While corporations dominate society and write the laws, each advance in technology is an opening for them to further restrict its users."),
    ("Richard M. Stallman", "I did write some code in Java once, but that was the island in Indonesia."),
    ("Richard M. Stallman", "People have a tendancy to be very scared of terrorists and not so scared of cars. But cars are a much bigger danger. The US declares war on terrorism. The cars have been killing thousands of Americans and still we don’t have a global war on cars. People are bad judges of how important various dangers are."),
    ("Ian Coldwater", "Look it up baby, you'll see my name on it."), // from https://twitter.com/IanColdwater/status/1292895288546545666
    ("Jessie Frazelle", "Building stuff wouldn't be fun if it wasn't hard."), // from https://twitter.com/jessfraz
    ("Julia Evans", "Asking good questions is a super important skill when writing software."), // from https://jvns.ca/blog/good-questions/
    ("Jane Silber", "Open source is important to our orgs as a talent pool; we need better representation of women."), //   https://www.azquotes.com/author/90343-Jane_Silber
    ("Margaret Heffernan", "Huge open source organizations like Red Hat and Mozilla manage the collaboration of hundreds of people who don't know one another and have spent no time hanging around the water cooler."), //   https://www.azquotes.com/author/16757-Margaret_Heffernan
    // The following quotes are from https://github.com/jkenley/hacktoberquote/blob/master/data/index.js
    ("Juan Veloz", "I based my entire career on collaboration- there is nothing better than succeeding with a team."),
    ("Helen Keller", "Alone we can do so little; together we can do so much."),
    ("Virgil Abloh", "I want to put culture on a track so that it becomes more inclusive, more open source."),
    ("Lydia Whitmore", "Collaborating is actually one of the strongest things you can do – to open yourself and your ideas up to someone else’s perspective."),
    ("Amit Ray", "Collaboration has no hierarchy. The Sun collaborates with soil to bring flowers on the earth."),
    ("Louisa May Alcott", "It takes two flints to make a fire."),
    ("Reid Hoffman", "No matter how brilliant your mind or strategy, if you're playing a solo game, you'll always lose out to a team."),
    // From Half-Life documentary https://www.youtube.com/watch?v=TbZ3HzvFEto&t=1723s
    ("Gabe Newell", "Late is just for a little while. Suck is forever right? We could try to force this thing (half-life) out the door, but that's not the company we want to be. That's not the people we want to be."),
];
