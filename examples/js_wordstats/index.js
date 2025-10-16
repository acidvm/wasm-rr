// Word Statistics Analyzer - Showcasing JavaScript features
// Features: closures, array methods, Map, generators, destructuring, arrow functions

// Generator function for creating ASCII art bars
function* barChart(value, maxValue) {
    const barLength = Math.floor((value / maxValue) * 20);
    for (let i = 0; i < barLength; i++) {
        yield '█';
    }
}

// Closure-based word analyzer factory
function createWordAnalyzer() {
    // Private state via closure
    const stats = {
        wordCount: 0,
        charCount: 0,
        uniqueWords: new Map()
    };

    return {
        analyze: (text) => {
            const words = text.toLowerCase()
                .split(/\s+/)
                .filter(word => word.length > 0)
                .map(word => word.replace(/[^a-z0-9]/g, ''));

            words.forEach(word => {
                if (word) {
                    stats.wordCount++;
                    stats.charCount += word.length;
                    stats.uniqueWords.set(
                        word,
                        (stats.uniqueWords.get(word) || 0) + 1
                    );
                }
            });
        },

        getStats: () => ({
            total: stats.wordCount,
            unique: stats.uniqueWords.size,
            avgLength: stats.wordCount > 0
                ? (stats.charCount / stats.wordCount).toFixed(2)
                : 0,
            frequencies: stats.uniqueWords
        })
    };
}

// Functional composition: chain operations
const compose = (...fns) => x => fns.reduceRight((v, f) => f(v), x);

// Higher-order function for formatting output
const formatWordEntry = (maxCount) => ([word, count]) => {
    const bar = Array.from(barChart(count, maxCount)).join('');
    const percentage = ((count / maxCount) * 100).toFixed(1);
    return `  ${word.padEnd(15)} ${count.toString().padStart(3)} ${bar} ${percentage}%`;
};

// Main execution
function main() {
    console.log("🎯 JavaScript Word Statistics Analyzer");
    console.log("=" .repeat(50));

    // Get environment/args - for demo, using some sample text
    // In actual usage with wasm-rr, this will come from the Arguments trace event
    const sampleText = "JavaScript is a versatile programming language. " +
                      "JavaScript supports functional programming and object-oriented programming. " +
                      "Many developers love JavaScript for web development. " +
                      "JavaScript can run on servers with Node.js and in browsers. " +
                      "The JavaScript ecosystem is vast and growing every day.";

    // Create analyzer instance
    const analyzer = createWordAnalyzer();

    // Analyze input text
    analyzer.analyze(sampleText);

    // Get statistics
    const stats = analyzer.getStats();

    // Display summary with emoji flair
    console.log("\n📊 Summary:");
    console.log(`   Total words: ${stats.total}`);
    console.log(`   Unique words: ${stats.unique}`);
    console.log(`   Avg length: ${stats.avgLength} chars`);

    // Display word frequencies (top 10)
    console.log("\n📈 Word Frequencies (Top 10):");

    const sortedWords = Array.from(stats.frequencies.entries())
        .sort(([,a], [,b]) => b - a)
        .slice(0, 10);

    if (sortedWords.length > 0) {
        const maxCount = sortedWords[0][1];
        const formatter = formatWordEntry(maxCount);

        sortedWords.forEach(entry => {
            console.log(formatter(entry));
        });
    }

    // Demonstrate array methods and functional programming
    console.log("\n🔤 Word Length Distribution:");
    const lengthGroups = Array.from(stats.frequencies.keys())
        .reduce((acc, word) => {
            const len = word.length;
            acc[len] = (acc[len] || 0) + 1;
            return acc;
        }, {});

    Object.entries(lengthGroups)
        .sort(([a], [b]) => Number(a) - Number(b))
        .forEach(([length, count]) => {
            const bar = '▓'.repeat(count);
            console.log(`   ${length} chars: ${bar} (${count})`);
        });

    console.log("\n✨ Analysis complete!");
}

// Run main function
try {
    main();
} catch (error) {
    console.error("Error:", error.message);
    throw error;
}
