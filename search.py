import lib

import re
import string
from urlparse import urlparse
import unittest

from stemming import porter2
from tags import *

# From AppEngine's SearchableModel
WORD_DELIMITER_REGEX = re.compile('[' + re.escape(string.punctuation) + ']')
FULL_TEXT_MIN_LENGTH = 2
FULL_TEXT_STOP_WORDS = frozenset([
   'a', 'about', 'according', 'accordingly', 'affected', 'affecting', 'after',
   'again', 'against', 'all', 'almost', 'already', 'also', 'although',
   'always', 'am', 'among', 'an', 'and', 'any', 'anyone', 'apparently', 'are',
   'arise', 'as', 'aside', 'at', 'away', 'be', 'became', 'because', 'become',
   'becomes', 'been', 'before', 'being', 'between', 'both', 'briefly', 'but',
   'by', 'came', 'can', 'cannot', 'certain', 'certainly', 'could', 'did', 'do',
   'does', 'done', 'during', 'each', 'either', 'else', 'etc', 'ever', 'every',
   'following', 'for', 'found', 'from', 'further', 'gave', 'gets', 'give',
   'given', 'giving', 'gone', 'got', 'had', 'hardly', 'has', 'have', 'having',
   'here', 'how', 'however', 'i', 'if', 'in', 'into', 'is', 'it', 'itself',
   'just', 'keep', 'kept', 'knowledge', 'largely', 'like', 'made', 'mainly',
   'make', 'many', 'might', 'more', 'most', 'mostly', 'much', 'must', 'nearly',
   'necessarily', 'neither', 'next', 'no', 'none', 'nor', 'normally', 'not',
   'noted', 'now', 'obtain', 'obtained', 'of', 'often', 'on', 'only', 'or',
   'other', 'our', 'out', 'owing', 'particularly', 'past', 'perhaps', 'please',
   'poorly', 'possible', 'possibly', 'potentially', 'predominantly', 'present',
   'previously', 'primarily', 'probably', 'prompt', 'promptly', 'put',
   'quickly', 'quite', 'rather', 'readily', 'really', 'recently', 'regarding',
   'regardless', 'relatively', 'respectively', 'resulted', 'resulting',
   'results', 'said', 'same', 'seem', 'seen', 'several', 'shall', 'should',
   'show', 'showed', 'shown', 'shows', 'significantly', 'similar', 'similarly',
   'since', 'slightly', 'so', 'some', 'sometime', 'somewhat', 'soon',
   'specifically', 'state', 'states', 'strongly', 'substantially',
   'successfully', 'such', 'sufficiently', 'than', 'that', 'the', 'their',
   'theirs', 'them', 'then', 'there', 'therefore', 'these', 'they', 'this',
   'those', 'though', 'through', 'throughout', 'to', 'too', 'toward', 'under',
   'unless', 'until', 'up', 'upon', 'use', 'used', 'usefully', 'usefulness',
   'using', 'usually', 'various', 'very', 'was', 'we', 'were', 'what', 'when',
   'where', 'whether', 'which', 'while', 'who', 'whose', 'why', 'widely',
   'will', 'with', 'within', 'without', 'would', 'yet', 'you'])

# Common but useless terms in URLs
# TODO: dump list of URLs and generate this properly
URL_STOP_WORDS = frozenset([
    'www', 'html', 'post', 'story', 'archive'])

class Results:
    def __init__(self, search_tokens, tags):
        self.search_tokens = search_tokens
        self.tags = tags

# Inspired by AppEngine's SearchableModel
def tokenize(text):
    """Tokenize an english phrase"""
    text = WORD_DELIMITER_REGEX.sub(' ', text)
    words = text.lower().split()
    words = set(unicode(w) for w in words if len(w) > FULL_TEXT_MIN_LENGTH)
    words -= FULL_TEXT_STOP_WORDS
    return words

def tokenize_url(url):
    url_tokens = tokenize(clean_url(url))
    url_tokens -= URL_STOP_WORDS
    return url_tokens

def tokenize_story(titles, tags, url):
    # Tokenize all the story titles, even the ones we are hiding from different scrapes
    tokens = set().union(*[tokenize(title) for title in titles])

    # Add the tags we've generated
    tokens = tokens.union(tags)

    # And the URL (but ignore common terms we don't care about)
    # TODO: Clean the url using the old code
    tokens = tokens.union(tokenize_url(url))

    return tokens

def clean_host(url):
    host = urlparse(url).netloc
    host = re.sub("^ww?w?[0-9]*\.", "", host)
    return host

def clean_url(url):
    url = urlparse(url).path

    # Chop off any extension-ish looking things at the end
    url = re.sub("\.[a-z]{1,5}$", '', url)
    # Chop the url into alphanumeric segments
    url = re.sub("[^A-Za-z0-9]", " ", url)

    return url
    
def generate_search_field(titles, existing_tags, url):
    # Compute all the tags given the existing tags and the tags we've extracted from the titles
    all_tags = [set(existing_tags)] + [set(extractTags(title)) for title in titles]
    tags = set.union(*all_tags)

    # Now compute the search tokens
    tokens = set([porter2.stem(x) for x in tokenize_story(titles, tags, url)])

    # Add the special host search token
    # TODO: we should probably add all domains up to the root (ie: blog.reddit.com+reddit.com)
    host = clean_host(url)
    tokens.add('host:%s' % host)

    # Add the host tag to the start
    tags = list(tags)
    tags.sort()
    tags.insert(0, host)

    return Results(tokens, tags)

class TestSearch(unittest.TestCase):
    def test_tokenize(self):
        self.assertEquals(set(['greatest', 'title']), tokenize('This is the greatest title ever'))

    def test_tokenize_url(self):
        self.assertEquals(set(['foo']), tokenize_url('http://google.com/foo'))
        self.assertEquals(set(['foo', 'bar']), tokenize_url('http://google.com/foo/bar'))
        self.assertEquals(set(['foo', 'bar']), tokenize_url('http://google.com/foo/bar.html'))

    def test_generate_search_field(self):
        res = generate_search_field(['first title', 'titled second javascript'], ['tag', 'bar', 'baz'], 'http://google.com/foo')
        self.assertEquals(set(['first', 'second', 'titl', 'javascript', 'foo', 'bar', 'baz', 'tag', 'host:google.com']), res.search_tokens)
        self.assertEquals(['google.com', 'bar', 'baz', 'javascript', 'tag'], res.tags)

if __name__ == '__main__':
    unittest.main()
