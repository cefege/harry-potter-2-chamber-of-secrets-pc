//===============================================================================
//  #72 Helga Hufflepuff
//===============================================================================

class  WCHufflepuff extends goldcards;

#EXEC TEXTURE IMPORT NAME=WizardCardHufflepuffTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Hufflepuffsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardHufflepuffBigTexture FILE=TEXTURES\menu\Folio\Cards\Hufflepuffbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

#EXEC TEXTURE IMPORT NAME=WizCardHelgaBigTextureLayer0 FILE=TEXTURES\menu\Folio\Cards\72_Helga_1.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardHelgaBigTextureLayer1 FILE=TEXTURES\menu\Folio\Cards\72_Helga_2.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardHelgaBigTextureLayer2 FILE=TEXTURES\menu\Folio\Cards\72_Helga_3.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Helga Hufflepuff";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=72
    skin(0)=Texture'HProps.skins.WizardCardHufflepuffTex0'
	textureBig=Texture'WizCardHufflepuffBigTexture'
	strDescriptionId="WizCard_0066"
	bIsLayered=true;
	textureLayers(0)=Texture'WizCardHelgaBigTextureLayer0';
	textureLayers(1)=Texture'WizCardHelgaBigTextureLayer1';
	textureLayers(2)=Texture'WizCardHelgaBigTextureLayer2';

}
