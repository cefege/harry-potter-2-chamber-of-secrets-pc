//===============================================================================
//  #100 Harry Potter: The boy who lived
//===============================================================================

class  WCPotter extends Goldcards;

#EXEC TEXTURE IMPORT NAME=WizardCardPotterTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Pottersmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardPotterBigTexture FILE=TEXTURES\menu\Folio\Cards\Potterbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardHarryBigTextureLayer0 FILE=TEXTURES\menu\Folio\Cards\100_Harry_1.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardHarryBigTextureLayer1 FILE=TEXTURES\menu\Folio\Cards\100_Harry_2.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardHarryBigTextureLayer2 FILE=TEXTURES\menu\Folio\Cards\100_Harry_3.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Harry Potter";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=100
    skin(0)=Texture'HProps.skins.WizardCardPotterTex0'
	textureBig=Texture'WizCardPotterBigTexture'
	strDescriptionId="WizCard_0038"
	bIsLayered=true;
	textureLayers(0)=Texture'WizCardHarryBigTextureLayer0';
	textureLayers(1)=Texture'WizCardHarryBigTextureLayer1';
	textureLayers(2)=Texture'WizCardHarryBigTextureLayer2';

}
