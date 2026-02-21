from imdb import IMDb

ia = IMDb()

movie = ia.get_movie("Rain Man")

print('Directors:')
for director in movie['directors']:
    print(director['name'])